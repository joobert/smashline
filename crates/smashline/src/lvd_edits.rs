//! Declarative LVD editing.
//!
//! The builders in this module mirror the LVD file's own hierarchy (as seen
//! in translated `.yml` dumps): `collisions` → `entry` / `col_flags` /
//! `vertices` / `normals` / `materials` → `line_material` / `line_flags`.
//! Each group is started by a lowercase function named after one of its yml
//! keys, so no word ever appears twice:
//!
//! ```ignore
//! Agent::new("pacman_firehydrant")
//!     .set_costume(vec![40])
//!     .lvd_edit(collisions()
//!         .entry(name("COL_curve1"))
//!         .col_flags(drop_through(true))
//!         .vertices([(0, -6.0, 0.0), (1, 6.0, 0.0)])
//!         .materials(line_material(GroundCollAttr::GROUND_COLL_ATTR_WOOD)
//!             .line_flags(throughable(true))))
//!     .install();
//! ```
//!
//! Every setter is optional — anything not set keeps its vanilla value. A
//! [`collisions()`] group applies to every collision unless narrowed with
//! [`entry(name(...))`](CollisionsEdit::entry); a materials group applies to
//! every material line unless narrowed with [`at`](MaterialsEdit::at).
//! Call [`Agent::lvd_edit`](crate::Agent::lvd_edit) multiple times (or chain
//! groups on [`lvd_edits()`]) to target several collisions differently.
//!
//! Everything here is re-exported at the crate root, so `use smashline::*;`
//! is all that's needed.

pub use crate::lvd::GroundCollAttr;

use crate::lvd::{Collision, LineFlags, LvdFile};

type LineFlagSetter = fn(&mut LineFlags, bool);

/// Starts an empty LVD edit; useful to group several [`collisions()`] edits
/// into one value. `Agent::lvd_edit(collisions()...)` works directly for the
/// common single-group case.
pub fn lvd_edits() -> LvdEdits {
    LvdEdits::default()
}

/// Starts a group of edits to the `collisions` list.
pub fn collisions() -> CollisionsEdit {
    CollisionsEdit::default()
}

/// Starts an `entry` selector matching the collision with this `name`
/// (e.g. `"COL_curve1"`).
pub fn name(name: impl Into<String>) -> EntryEdit {
    EntryEdit {
        name: Some(name.into()),
    }
}

/// Starts a `materials` group with `line_material` (the ground attribute:
/// iron, wood, ice, ...) set.
pub fn line_material(material: GroundCollAttr) -> MaterialsEdit {
    MaterialsEdit::default().line_material(material)
}

/// The root of a declarative LVD edit.
#[derive(Default, Clone)]
pub struct LvdEdits {
    collisions: Vec<CollisionsEdit>,
}

impl LvdEdits {
    /// Adds a group of edits to the file's `collisions` list. May be called
    /// multiple times, e.g. to target different collisions by name.
    pub fn collisions(mut self, edit: CollisionsEdit) -> Self {
        self.collisions.push(edit);
        self
    }

    pub(crate) fn apply(&self, file: &mut LvdFile) {
        for edit in &self.collisions {
            edit.apply(file);
        }
    }
}

impl From<CollisionsEdit> for LvdEdits {
    fn from(edit: CollisionsEdit) -> Self {
        LvdEdits::default().collisions(edit)
    }
}

/// Selects which collision a [`collisions()`] group applies to, via the
/// collision's `entry` fields. Built with [`name()`].
#[derive(Default, Clone)]
pub struct EntryEdit {
    name: Option<String>,
}

/// Edits applied to entries of the `collisions` list — all of them by
/// default, or specific ones selected with [`entry`](Self::entry).
/// Started with [`collisions()`].
#[derive(Default, Clone)]
pub struct CollisionsEdit {
    entry: EntryEdit,
    col_flags: Vec<ColFlagsEdit>,
    vertices: Vec<(usize, f32, f32)>,
    normals: Vec<(usize, f32, f32)>,
    materials: Vec<MaterialsEdit>,
}

impl CollisionsEdit {
    /// Restricts this group to collisions matching the `entry` selector,
    /// e.g. `.entry(name("COL_curve1"))`.
    pub fn entry(mut self, entry: EntryEdit) -> Self {
        self.entry = entry;
        self
    }

    /// Edits the collision's `col_flags`, e.g. `.col_flags(drop_through(true))`.
    pub fn col_flags(mut self, edit: ColFlagsEdit) -> Self {
        self.col_flags.push(edit);
        self
    }

    /// Moves vertices by index: `.vertices([(0, -6.0, 0.0), (1, 6.0, 0.0)])`
    /// (absolute world coordinates).
    pub fn vertices(mut self, points: impl IntoIterator<Item = (usize, f32, f32)>) -> Self {
        self.vertices.extend(points);
        self
    }

    /// Sets normals by index: `.normals([(0, 0.0, 1.0)])`.
    pub fn normals(mut self, points: impl IntoIterator<Item = (usize, f32, f32)>) -> Self {
        self.normals.extend(points);
        self
    }

    /// Edits the collision's `materials` (the per-segment line properties),
    /// e.g. `.materials(line_material(...).line_flags(throughable(true)))`.
    /// A bare line-flag edit works too: `.materials(throughable(true))`.
    /// May be called multiple times.
    pub fn materials(mut self, edit: impl Into<MaterialsEdit>) -> Self {
        self.materials.push(edit.into());
        self
    }

    fn apply(&self, file: &mut LvdFile) {
        let mut matched = false;

        for collision in file.collisions.iter_mut() {
            if let Some(name) = &self.entry.name {
                if &collision.entry.name != name {
                    continue;
                }
            }
            matched = true;

            for edit in &self.col_flags {
                edit.apply(collision);
            }

            for &(index, x, y) in &self.vertices {
                if let Some(vertex) = collision.vertices.get_mut(index) {
                    vertex.x = x;
                    vertex.y = y;
                } else {
                    println!(
                        "[smashline::lvd] vertex index {index} is out of bounds for collision '{}'",
                        collision.entry.name
                    );
                }
            }

            for &(index, x, y) in &self.normals {
                if let Some(normal) = collision.normals.get_mut(index) {
                    normal.x = x;
                    normal.y = y;
                } else {
                    println!(
                        "[smashline::lvd] normal index {index} is out of bounds for collision '{}'",
                        collision.entry.name
                    );
                }
            }

            for edit in &self.materials {
                edit.apply(collision);
            }
        }

        if !matched {
            if let Some(name) = &self.entry.name {
                println!("[smashline::lvd] no collision named '{name}' found in this LVD file");
            }
        }
    }
}

macro_rules! col_flags_api {
    ($($(#[$doc:meta])* $name:ident),* $(,)?) => {
        /// Edits to a collision's `col_flags`. Started with any of the flag
        /// functions, e.g. [`drop_through()`].
        #[derive(Default, Clone)]
        pub struct ColFlagsEdit {
            $($name: Option<bool>,)*
        }

        impl ColFlagsEdit {
            $(
                $(#[$doc])*
                pub fn $name(mut self, value: bool) -> Self {
                    self.$name = Some(value);
                    self
                }
            )*

            fn apply(&self, collision: &mut Collision) {
                $(
                    if let Some(value) = self.$name {
                        collision.col_flags.$name = value;
                    }
                )*
            }
        }

        $(
            /// Starts a `col_flags` edit with the `
            #[doc = stringify!($name)]
            /// ` flag set.
            pub fn $name(value: bool) -> ColFlagsEdit {
                ColFlagsEdit::default().$name(value)
            }
        )*
    }
}

col_flags_api! {
    flag1,
    /// Whether the collision is rigged to an animated bone.
    rig_col,
    flag3,
    /// Whether characters can press down to drop through the collision.
    drop_through,
}

/// Edits applied to entries of a collision's `materials` list — all of them
/// by default, or a single line selected with [`at`](Self::at).
/// Started with [`line_material()`] or any line-flag function.
#[derive(Default, Clone)]
pub struct MaterialsEdit {
    index: Option<usize>,
    line_material: Option<GroundCollAttr>,
    line_flags: Vec<LineFlagsEdit>,
}

impl MaterialsEdit {
    /// Restricts this group to the material line at `index`. By default the
    /// edits apply to every line of the collision.
    pub fn at(mut self, index: usize) -> Self {
        self.index = Some(index);
        self
    }

    /// Sets `line_material` — the ground attribute (iron, wood, ice, ...).
    pub fn line_material(mut self, material: GroundCollAttr) -> Self {
        self.line_material = Some(material);
        self
    }

    /// Edits the material's `line_flags`, e.g.
    /// `.line_flags(throughable(true).breakable(false))`.
    pub fn line_flags(mut self, edit: LineFlagsEdit) -> Self {
        self.line_flags.push(edit);
        self
    }

    fn apply(&self, collision: &mut Collision) {
        if let Some(index) = self.index {
            if index >= collision.materials.len() {
                println!(
                    "[smashline::lvd] material index {index} is out of bounds for collision '{}'",
                    collision.entry.name
                );
                return;
            }
        }

        for (index, material) in collision.materials.iter_mut().enumerate() {
            if let Some(target) = self.index {
                if index != target {
                    continue;
                }
            }

            if let Some(line_material) = self.line_material {
                material.line_material = line_material;
            }

            for edit in &self.line_flags {
                edit.apply(&mut material.line_flags);
            }
        }
    }
}

impl From<LineFlagsEdit> for MaterialsEdit {
    fn from(edit: LineFlagsEdit) -> Self {
        MaterialsEdit::default().line_flags(edit)
    }
}

macro_rules! line_flags_api {
    ($($name:ident => $setter:ident),* $(,)?) => {
        /// Edits to a material's `line_flags`. Started with any of the flag
        /// functions, e.g. `throughable(true).breakable(false)`.
        #[derive(Default, Clone)]
        pub struct LineFlagsEdit {
            flags: Vec<(LineFlagSetter, bool)>,
        }

        impl LineFlagsEdit {
            $(
                /// Sets the `
                #[doc = stringify!($name)]
                /// ` flag.
                pub fn $name(mut self, value: bool) -> Self {
                    self.flags.push((
                        (|flags: &mut LineFlags, v: bool| flags.$setter(v)) as LineFlagSetter,
                        value,
                    ));
                    self
                }
            )*

            fn apply(&self, flags: &mut LineFlags) {
                for &(setter, value) in &self.flags {
                    setter(flags, value);
                }
            }
        }

        $(
            /// Starts a `line_flags` edit with the `
            #[doc = stringify!($name)]
            /// ` flag set.
            pub fn $name(value: bool) -> LineFlagsEdit {
                LineFlagsEdit::default().$name(value)
            }
        )*
    }
}

line_flags_api! {
    length_zero => set_length_zero,
    pacman_final_ignore => set_pacman_final_ignore,
    fall => set_fall,
    ignore_ray_check => set_ignore_ray_check,
    dive => set_dive,
    unpaintable => set_unpaintable,
    item => set_item,
    ignore_fighter_other => set_ignore_fighter_other,
    right => set_right,
    left => set_left,
    upper => set_upper,
    under => set_under,
    not_attach => set_not_attach,
    throughable => set_throughable,
    hang_l => set_hang_l,
    hang_r => set_hang_r,
    ignore_link_from_left => set_ignore_link_from_left,
    cloud => set_cloud,
    ignore_link_from_right => set_ignore_link_from_right,
    not_expand_near_search => set_not_expand_near_search,
    ignore => set_ignore,
    breakable => set_breakable,
    immediate_relanding_ban => set_immediate_relanding_ban,
    ignore_line_type1 => set_ignore_line_type1,
    pickel_block => set_pickel_block,
    deceleration => set_deceleration,
    virtual_hit_line_up => set_virtual_hit_line_up,
    virtual_hit_line_left => set_virtual_hit_line_left,
    virtual_hit_line_right => set_virtual_hit_line_right,
    virtual_hit_line_down => set_virtual_hit_line_down,
    virtual_wall_hit_line => set_virtual_wall_hit_line,
    ignore_boss => set_ignore_boss,
}

/// Derives the conventional LVD path for an agent name like
/// `"pacman_firehydrant"` → `"fighter/pacman/model/firehydrant/c00/firehydrant.lvd"`.
pub(crate) fn inferred_lvd_path(agent_name: &str) -> String {
    let Some((owner, article)) = agent_name.split_once('_') else {
        panic!(
            "smashline: cannot infer an LVD path from agent '{agent_name}' \
             (expected '<fighter>_<article>'); use lvd_edit_file to provide the path explicitly"
        );
    };

    format!("fighter/{owner}/model/{article}/c00/{article}.lvd")
}
