use std::collections::BTreeMap;

use locks::RwLock;
use resources::{
    smash_arc::{hash40, ArcLookup, Hash40 as ArcHash40, Region},
    types::{FilesystemInfo, ResServiceNX},
};
use smashline::{Costume, Hash40, LvdEditCallback};

use crate::create_agent::{
    LOWERCASE_FIGHTER_NAMES, LOWERCASE_WEAPON_NAMES, LOWERCASE_WEAPON_OWNER_NAMES,
};

pub struct LvdEdit {
    pub agent: Hash40,
    /// smash-arc hash of the full file path, e.g. "fighter/pacman/param/firehydrant.lvd"
    pub file: u64,
    /// The original path string, kept for diagnostics.
    pub path: String,
    pub costume: Costume,
    pub callback: LvdEditCallback,
    pub user_data: *const (),
}

// The user data is only ever handed back to the callback it was registered
// with; the plugin itself never dereferences it.
unsafe impl Send for LvdEdit {}
unsafe impl Sync for LvdEdit {}

pub static LVD_EDITS: RwLock<Vec<LvdEdit>> = RwLock::new(Vec::new());

/// Pristine copies of each edited file, saved before the first patch so that
/// non-marked slots can be restored to vanilla data.
static LVD_ORIGINALS: RwLock<BTreeMap<u64, Vec<u8>>> = RwLock::new(BTreeMap::new());

fn region() -> Region {
    ResServiceNX::instance()
        .map(|service| Region::from(service.region_idx as usize + 1))
        .unwrap_or(Region::UsEnglish)
}

/// Returns the resident decompressed data of a loaded file, or `None` if the
/// file is not currently loaded by the resource service.
fn loaded_file_slice(file: u64, path: &str) -> Option<&'static mut [u8]> {
    let Some(fs) = FilesystemInfo::instance() else {
        println!("[smashline::lvd] filesystem info unavailable for '{path}'");
        return None;
    };
    let arc = fs.arc();
    let hash = ArcHash40(file);

    let Ok(path_index) = arc.get_file_path_index_from_hash(hash) else {
        println!("[smashline::lvd] '{path}' does not exist in data.arc");
        return None;
    };
    let loaded = fs.get_loaded_filepaths().get(path_index.0 as usize)?;
    if loaded.is_loaded == 0 {
        println!("[smashline::lvd] '{path}' is not loaded yet, skipping");
        return None;
    }

    let data = fs.get_loaded_datas().get(loaded.loaded_data_index as usize)?;
    if data.data.is_null() {
        println!("[smashline::lvd] '{path}' has no resident data, skipping");
        return None;
    }

    let size = match arc.get_file_data_from_hash(hash, region()) {
        Ok(file_data) => file_data.decomp_size as usize,
        Err(_) => {
            println!("[smashline::lvd] failed to look up file data for '{path}'");
            return None;
        }
    };

    Some(unsafe { std::slice::from_raw_parts_mut(data.data.cast_mut(), size) })
}

pub fn install_lvd_edit(agent: Hash40, costume: Costume, path: &str, callback: LvdEditCallback, user_data: *const ()) {
    println!(
        "[smashline::lvd] registered edit for agent {:#x} on '{}' (slots: {:?})",
        agent.0,
        path,
        costume.as_slice()
    );

    LVD_EDITS.write().push(LvdEdit {
        agent,
        file: hash40(path).as_u64(),
        path: path.to_string(),
        costume,
        callback,
        user_data,
    });
}

/// Rewrites every file registered by an edit matching `predicate` back to its
/// original bytes, then runs the callbacks whose costume matches `costume`.
fn apply_edits_filtered(predicate: impl Fn(&LvdEdit) -> bool, costume: i32) {
    let edits = LVD_EDITS.read();

    let mut files: Vec<u64> = edits
        .iter()
        .filter(|e| predicate(e))
        .map(|e| e.file)
        .collect();
    files.sort_unstable();
    files.dedup();

    for file in files {
        let path = edits
            .iter()
            .find(|e| e.file == file)
            .map(|e| e.path.as_str())
            .unwrap_or("<unknown>");

        let Some(buffer) = loaded_file_slice(file, path) else {
            continue;
        };

        {
            let mut originals = LVD_ORIGINALS.write();
            let original = originals.entry(file).or_insert_with(|| buffer.to_vec());
            buffer.copy_from_slice(original);
        }

        let mut applied = 0usize;
        for edit in edits.iter().filter(|e| predicate(e) && e.file == file) {
            let has_costume = crate::utils::has_costume(edit.agent, costume);
            let c = edit.costume.as_slice();

            if has_costume && !c.contains(&(costume as usize)) {
                continue;
            }

            if !has_costume && !c.is_empty() {
                continue;
            }

            unsafe {
                (edit.callback)(buffer.as_mut_ptr(), buffer.len(), edit.user_data);
            }
            applied += 1;
        }

        println!(
            "[smashline::lvd] '{}' rebased ({} bytes), {} edit(s) applied for slot {}",
            path,
            buffer.len(),
            applied,
            costume
        );
    }
}

/// Applies the edits registered for a spawning battle object. Edits are
/// matched against the object's own agent hash, and for weapons also against
/// the owner fighter's hash, so registrations on either work.
///
/// This runs per spawned instance, which is what keeps a shared LVD file
/// correct when fighters on marked and unmarked slots are in the same match:
/// each spawn re-bases the file and re-applies only its own slot's edits.
pub fn apply_lvd_edits_for_spawn(agent: Hash40, object_kind: i32, is_weapon: bool, costume: i32) {
    if LVD_EDITS.read().is_empty() {
        return;
    }

    let mut candidates = vec![agent];

    if is_weapon {
        if let Some(owner) = LOWERCASE_WEAPON_OWNER_NAMES.get(object_kind as usize) {
            candidates.push(Hash40::new(owner));
        }
    }

    apply_edits_filtered(|e| candidates.contains(&e.agent), costume);
}

/// Applies the edits registered for a fighter and all of the weapons it owns
/// (vanilla and cloned), e.g. when the fighter itself initializes.
pub fn apply_lvd_edits_for_fighter(kind: i32, costume: i32) {
    if LVD_EDITS.read().is_empty() {
        return;
    }

    let Some(fighter_name) = LOWERCASE_FIGHTER_NAMES.get(kind as usize) else {
        return;
    };

    let mut candidates = vec![Hash40::new(fighter_name)];

    for (weapon_id, owner) in LOWERCASE_WEAPON_OWNER_NAMES.iter().enumerate() {
        if owner == fighter_name {
            if let Some(name) = LOWERCASE_WEAPON_NAMES.get(weapon_id) {
                candidates.push(Hash40::new(&format!("{owner}_{name}")));
            }
        }
    }

    for agent in crate::cloning::weapons::NEW_AGENTS.read().iter() {
        if agent.owner_id == kind {
            candidates.push(Hash40::new(&format!(
                "{}_{}",
                agent.owner_name, agent.article_name
            )));
        }
    }

    apply_edits_filtered(|e| candidates.contains(&e.agent), costume);
}

pub fn remove_by_plugin_range(start: usize, end: usize) {
    let mut edits = LVD_EDITS.write();
    let working = std::mem::take(&mut *edits);
    *edits = working
        .into_iter()
        .filter(|edit| !(start..end).contains(&(edit.callback as *const () as usize)))
        .collect();
}
