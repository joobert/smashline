use crate::{AsAgentIdent, AsHash40, Costume, LvdEdits, ObjectEvent, Priority, StatusLine};

pub type AcmdFunction = unsafe extern "C" fn(&mut crate::L2CAgentBase);

pub unsafe extern "C" fn acmd_stub(_agent: &mut crate::L2CAgentBase) {}

pub type StateFunction<T> = unsafe extern "C" fn(&mut T);

mod __sealed {
    pub trait Sealed {}
}

pub trait StatusLineMarker: __sealed::Sealed {
    type Function<T>;
    type LineFunction<T>;
    const LINE: StatusLine;

    unsafe fn cast_function<T>(func: Self::Function<T>) -> *const ();
    unsafe fn cast_line_function<T>(func: Self::LineFunction<T>) -> *const ();

    unsafe fn cast_ptr<T>(ptr: *const ()) -> Self::Function<T>;
}

macro_rules! markers {
    ($($name:ident($($func:tt)*));*) => {
        $(
            pub struct $name;
            impl __sealed::Sealed for $name {}
            impl StatusLineMarker for $name {
                type Function<T> = $($func)* -> crate::L2CValue;
                type LineFunction<T> = $($func)*;
                const LINE: StatusLine = StatusLine::$name;

                unsafe fn cast_function<T>(func: Self::Function<T>) -> *const () {
                    func as *const ()
                }

                unsafe fn cast_line_function<T>(func: Self::LineFunction<T>) -> *const () {
                    func as *const ()
                }

                unsafe fn cast_ptr<T>(ptr: *const ()) -> Self::Function<T> {
                    std::mem::transmute(ptr)
                }
            }
        )*
    }
}

markers! {
    Pre(unsafe extern "C" fn(&mut T));
    Main(unsafe extern "C" fn(&mut T));
    End(unsafe extern "C" fn(&mut T));
    Init(unsafe extern "C" fn(&mut T));
    Exec(unsafe extern "C" fn(&mut T));
    ExecStop(unsafe extern "C" fn(&mut T));
    Post(unsafe extern "C" fn(&mut T));
    Exit(unsafe extern "C" fn(&mut T));
    MapCorrection(unsafe extern "C" fn(&mut T));
    FixCamera(unsafe extern "C" fn(&mut T));
    FixPosSlow(unsafe extern "C" fn(&mut T));
    CheckDamage(unsafe extern "C" fn(&mut T, &crate::L2CValue));
    CheckAttack(unsafe extern "C" fn(&mut T, &crate::L2CValue, &crate::L2CValue));
    OnChangeLr(unsafe extern "C" fn(&mut T, &crate::L2CValue, &crate::L2CValue));
    LeaveStop(unsafe extern "C" fn(&mut T, &crate::L2CValue, &crate::L2CValue));
    NotifyEventGimmick(unsafe extern "C" fn(&mut T, &crate::L2CValue));
    CalcParam(unsafe extern "C" fn(&mut T))
}

struct AcmdScript {
    category: crate::Acmd,
    replaces: crate::Hash40,
    function: AcmdFunction,
    priority: Priority
}

struct LineCallback {
    line: StatusLine,
    function: *const (),
}

struct StateCallback {
    event: ObjectEvent,
    function: *const (),
}

struct StatusScript {
    line: StatusLine,
    kind: i32,
    function: *const (),
}

struct WeaponCount {
    fighter_id: i32,
    generate_article_id: i32,
    count: i32,
}

struct LvdEdit {
    path: String,
    user_data: *const (),
}

pub struct Agent {
    kind_hash: crate::Hash40,
    name: Option<String>,
    costume: Costume,
    acmd: Vec<AcmdScript>,
    lines: Vec<LineCallback>,
    status: Vec<StatusScript>,
    events: Vec<StateCallback>,
    weapon_counts: Vec<WeaponCount>,
    lvd_edits: Vec<LvdEdit>,
}

impl Agent {
    pub fn new(agent: impl AsAgentIdent) -> Self {
        let (kind_hash, name) = agent.as_agent_ident();
        Self {
            kind_hash,
            name,
            costume: Costume::default(),
            acmd: vec![],
            lines: vec![],
            status: vec![],
            events: vec![],
            weapon_counts: vec![],
            lvd_edits: vec![],
        }
    }

    pub fn set_costume(&mut self, costumes: Vec<usize>) -> &mut Self {
        self.costume = Costume::from_vec(costumes);
        self
    }

    pub fn acmd(&mut self, name: &str, function: AcmdFunction, priority: Priority) -> &mut Self {
        let category = if name.starts_with("game") {
            Some(crate::Acmd::Game)
        }
        else if name.starts_with("effect") {
            Some(crate::Acmd::Effect)
        }
        else if name.starts_with("sound") {
            Some(crate::Acmd::Sound)
        }
        else if name.starts_with("expression") {
            Some(crate::Acmd::Expression)
        }
        else {
            None
        };
        if category.is_some() {
            self.acmd.push(AcmdScript {
                category: category.unwrap(),
                replaces: name.as_hash40(),
                function,
                priority,
            });
        }
        else {
            println!("ACMD Category for {} could not be found! Skipping...", name);
        }
        self
    }

    pub fn game_acmd(&mut self, name: impl AsHash40, function: AcmdFunction, priority: Priority) -> &mut Self {
        self.acmd.push(AcmdScript {
            category: crate::Acmd::Game,
            replaces: name.as_hash40(),
            function,
            priority
        });
        self
    }

    pub fn effect_acmd(&mut self, name: impl AsHash40, function: AcmdFunction, priority: Priority) -> &mut Self {
        self.acmd.push(AcmdScript {
            category: crate::Acmd::Effect,
            replaces: name.as_hash40(),
            function,
            priority
        });
        self
    }

    pub fn sound_acmd(&mut self, name: impl AsHash40, function: AcmdFunction, priority: Priority) -> &mut Self {
        self.acmd.push(AcmdScript {
            category: crate::Acmd::Sound,
            replaces: name.as_hash40(),
            function,
            priority
        });

        self
    }

    pub fn expression_acmd(&mut self, name: impl AsHash40, function: AcmdFunction, priority: Priority) -> &mut Self {
        self.acmd.push(AcmdScript {
            category: crate::Acmd::Expression,
            replaces: name.as_hash40(),
            function,
            priority
        });

        self
    }

    #[allow(unused_variables)]
    pub fn status<M: StatusLineMarker, T>(
        &mut self,
        line: M,
        kind: i32,
        function: M::Function<T>,
    ) -> &mut Self {
        self.status.push(StatusScript {
            line: M::LINE,
            kind,
            function: unsafe { M::cast_function(function) },
        });
        self
    }

    #[allow(unused)]
    pub fn on_line<M: StatusLineMarker, T>(
        &mut self,
        line: M,
        function: M::LineFunction<T>,
    ) -> &mut Self {
        self.lines.push(LineCallback {
            line: M::LINE,
            function: unsafe { M::cast_line_function(function) },
        });
        self
    }

    pub fn on_init<T>(&mut self, func: StateFunction<T>) -> &mut Self {
        self.events.push(StateCallback {
            event: ObjectEvent::Initialize,
            function: func as *const (),
        });

        self
    }

    pub fn on_fini<T>(&mut self, func: StateFunction<T>) -> &mut Self {
        self.events.push(StateCallback {
            event: ObjectEvent::Finalize,
            function: func as *const (),
        });

        self
    }

    pub fn on_start<T>(&mut self, func: StateFunction<T>) -> &mut Self {
        self.events.push(StateCallback {
            event: ObjectEvent::Start,
            function: func as *const (),
        });

        self
    }

    /// Applies declarative edits to this agent's LVD file (e.g. article
    /// collision data such as pacman's firehydrant), with the path inferred
    /// from the agent name: `"pacman_firehydrant"` →
    /// `"fighter/pacman/model/firehydrant/c00/firehydrant.lvd"`.
    ///
    /// The edits are re-applied from the vanilla data every time the agent
    /// spawns or activates. If a costume was set with `set_costume`, they
    /// only apply to instances on those costume slots — other slots keep the
    /// unmodified file.
    ///
    /// ```ignore
    /// Agent::new("pacman_firehydrant")
    ///     .set_costume(vec![40])
    ///     .lvd_edit(collisions()
    ///         .entry(name("COL_curve1"))
    ///         .materials(line_material(GroundCollAttr::GROUND_COLL_ATTR_WOOD)
    ///             .line_flags(throughable(true))))
    ///     .install();
    /// ```
    pub fn lvd_edit(&mut self, edits: impl Into<LvdEdits>) -> &mut Self {
        let path = match &self.name {
            Some(name) => crate::lvd_edits::inferred_lvd_path(name),
            None => panic!(
                "smashline: Agent::lvd_edit requires the agent to be created from a name \
                 string so the LVD path can be inferred; use lvd_edit_file instead"
            ),
        };

        self.lvd_edit_file(path, edits)
    }

    /// Same as [`Agent::lvd_edit`], but with an explicit LVD file path for
    /// files that don't follow the standard layout.
    pub fn lvd_edit_file(
        &mut self,
        lvd_path: impl Into<String>,
        edits: impl Into<LvdEdits>,
    ) -> &mut Self {
        let edits = edits.into();
        self.lvd_edit_with(lvd_path, move |lvd| edits.apply(lvd))
    }

    /// Escape hatch for edits [`LvdEdits`] can't express: the closure receives
    /// the fully parsed LVD file and may change any values, but must not add
    /// or remove entries (the file size has to stay the same).
    pub fn lvd_edit_with(
        &mut self,
        lvd_path: impl Into<String>,
        edit: impl Fn(&mut crate::lvd::LvdFile) + 'static,
    ) -> &mut Self {
        self.lvd_edits.push(LvdEdit {
            path: lvd_path.into(),
            user_data: crate::leak_lvd_closure(edit),
        });

        self
    }

    /// Overrides the max spawn count of one of `fighter_id`'s articles, by generate article id
    /// (cloned articles included). If a costume was set with `set_costume`, the override only
    /// applies to fighters on those costume slots.
    pub fn update_weapon_count(&mut self, fighter_id: i32, generate_article_id: i32, count: i32) -> &mut Self {
        self.weapon_counts.push(WeaponCount { fighter_id, generate_article_id, count });

        self
    }

    pub fn on_end<T>(&mut self, func: StateFunction<T>) -> &mut Self {
        self.events.push(StateCallback {
            event: ObjectEvent::End,
            function: func as *const (),
        });

        self
    }

    pub fn install(&self) {
        for acmd in self.acmd.iter() {
            crate::api::install_acmd_script_costume(
                self.kind_hash,
                self.costume,
                acmd.replaces,
                acmd.category,
                acmd.priority,
                acmd.function,
            );
        }

        for status in self.status.iter() {
            crate::api::install_status_script_costume(
                Some(self.kind_hash),
                self.costume,
                status.line,
                status.kind,
                status.function,
            );
        }

        for line in self.lines.iter() {
            crate::api::install_line_callback_costume(
                Some(self.kind_hash),
                self.costume,
                line.line,
                line.function
            );
        }

        for event in self.events.iter() {
            crate::api::install_state_callback_costume(
                Some(self.kind_hash),
                self.costume,
                event.event,
                event.function as *const (),
            );
        }

        for weapon_count in self.weapon_counts.iter() {
            crate::update_weapon_count_costume(
                weapon_count.fighter_id,
                self.costume,
                weapon_count.generate_article_id,
                weapon_count.count,
            );
        }

        for lvd_edit in self.lvd_edits.iter() {
            crate::api::install_lvd_edit_costume(
                Some(self.kind_hash),
                self.costume,
                lvd_edit.path.clone(),
                crate::lvd_edit_trampoline,
                lvd_edit.user_data,
            );
        }
    }
}
