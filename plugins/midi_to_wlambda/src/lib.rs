use atomic_float::AtomicF32;
use nih_plug::prelude::*;
use nih_plug_egui::{create_egui_editor, egui, widgets, EguiState};
use rtrb::{Consumer, Producer, RingBuffer};
use std::sync::Arc;
use std::sync::Mutex;
use wlambda::rpc_helper::{RPCHandle, RPCHandleStopper};
use wlambda::threads::AValChannel;
use wlambda::vval::{VValFun, VValUserData};
use wlambda::{Env, EvalContext, StackAction, VVal};

#[derive(Clone)]
struct AtomicFloatVec {
    v: Vec<Arc<AtomicF32>>,
}

impl AtomicFloatVec {
    pub fn new(len: usize) -> Self {
        let mut v = vec![];
        for _ in 0..len {
            v.push(Arc::new(AtomicF32::new(0.0)));
        }
        Self { v: v }
    }

    pub fn set(&self, idx: usize, v: f32) {
        if idx < self.v.len() {
            self.v[idx].store(v, std::sync::atomic::Ordering::Relaxed);
        }
    }
}

impl VValUserData for AtomicFloatVec {
    fn s(&self) -> String {
        format!("$<AtomicFloatVec>")
    }
    fn as_any(&mut self) -> &mut dyn std::any::Any {
        self
    }
    fn clone_ud(&self) -> Box<dyn VValUserData> {
        Box::new(self.clone())
    }
    fn as_thread_safe_usr(&mut self) -> Option<Box<dyn wlambda::threads::ThreadSafeUsr>> {
        Some(Box::new(self.clone()))
    }

    fn call_method(&self, key: &str, env: &mut Env) -> Result<VVal, StackAction> {
        let argv = env.argv_ref();
        match key {
            "get" => {
                if argv.len() != 1 {
                    return Err(StackAction::panic_str(
                        "get method expects 1 arguments: index".to_string(),
                        None,
                        env.argv(),
                    ));
                }

                let idx = argv[0].i() as usize;
                Ok(if idx < self.v.len() {
                    VVal::Flt(self.v[idx].load(std::sync::atomic::Ordering::Relaxed) as f64)
                } else {
                    VVal::Flt(0.0)
                })
            }
            _ => Err(StackAction::panic_str(
                format!("unknown method called: {}", key),
                None,
                env.argv(),
            )),
        }
    }
}

impl wlambda::threads::ThreadSafeUsr for AtomicFloatVec {
    fn to_vval(&self) -> VVal {
        VVal::Usr(Box::new(AtomicFloatVec { v: self.v.clone() }))
    }
}

/// This is mostly identical to the gain example, minus some fluff, and with a GUI.
#[allow(dead_code)]
pub struct Midi2WLambda {
    params: Arc<Midi2WLambdaParams>,

    wl_handle: RPCHandle,
    wl_handle_stopper: Option<RPCHandleStopper>,

    gui_log_channel: AValChannel,

    param_vval_vec: AtomicFloatVec,

    task_queue: Option<Producer<M2WTask>>,
    task_queue2: Arc<Mutex<Option<Producer<M2WTask>>>>,
}

#[derive(Params)]
pub struct Midi2WLambdaParams {
    /// The editor state, saved together with the parameter state so the custom scaling can be
    /// restored.
    #[persist = "editor-state"]
    editor_state: Arc<EguiState>,

    #[persist = "wlambda-init-code"]
    wlambda_init_code: Arc<Mutex<String>>,

    #[persist = "wlambda-init-path"]
    wlambda_init_path: Arc<Mutex<String>>,

    #[persist = "wlambda-code"]
    wlambda_code: Arc<Mutex<String>>,

    #[persist = "wlambda-path"]
    wlambda_path: Arc<Mutex<String>>,

    #[id = "l1_val"]
    pub l1_val: FloatParam,
    #[id = "l1_clr"]
    pub l1_clr: FloatParam,
    #[id = "l1_x"]
    pub l1_x: FloatParam,

    #[id = "l2_val"]
    pub l2_val: FloatParam,
    #[id = "l2_clr"]
    pub l2_clr: FloatParam,
    #[id = "l2_x"]
    pub l2_x: FloatParam,

    #[id = "l3_val"]
    pub l3_val: FloatParam,
    #[id = "l3_clr"]
    pub l3_clr: FloatParam,
    #[id = "l3_x"]
    pub l3_x: FloatParam,

    #[id = "l4_val"]
    pub l4_val: FloatParam,
    #[id = "l4_clr"]
    pub l4_clr: FloatParam,
    #[id = "l4_x"]
    pub l4_x: FloatParam,

    #[id = "l5_val"]
    pub l5_val: FloatParam,
    #[id = "l5_clr"]
    pub l5_clr: FloatParam,
    #[id = "l5_x"]
    pub l5_x: FloatParam,
}

impl Default for Midi2WLambda {
    fn default() -> Self {
        let wl_handle = RPCHandle::new();

        Self {
            params: Arc::new(Midi2WLambdaParams::default()),

            gui_log_channel: AValChannel::new_direct(),

            param_vval_vec: AtomicFloatVec::new(15),

            task_queue: None,
            task_queue2: Arc::new(Mutex::new(None)),

            wl_handle,
            wl_handle_stopper: None,
        }
    }
}

impl Default for Midi2WLambdaParams {
    fn default() -> Self {
        Self {
            editor_state: EguiState::from_size(1200, 500),

            l1_val: FloatParam::new(
                "Lamp1 Value",
                0.0,
                FloatRange::Linear { min: 0.0, max: 1.0 },
            )
            .with_smoother(SmoothingStyle::Linear(5.0)),
            l1_clr: FloatParam::new(
                "Lamp1 Color",
                0.0,
                FloatRange::Linear { min: 0.0, max: 1.0 },
            )
            .with_smoother(SmoothingStyle::Linear(5.0)),
            l1_x: FloatParam::new("Lamp1 X", 0.0, FloatRange::Linear { min: 0.0, max: 1.0 })
                .with_smoother(SmoothingStyle::Linear(5.0)),

            l2_val: FloatParam::new(
                "Lamp1 Value",
                0.0,
                FloatRange::Linear { min: 0.0, max: 1.0 },
            )
            .with_smoother(SmoothingStyle::Linear(5.0)),
            l2_clr: FloatParam::new(
                "Lamp1 Color",
                0.0,
                FloatRange::Linear { min: 0.0, max: 1.0 },
            )
            .with_smoother(SmoothingStyle::Linear(5.0)),
            l2_x: FloatParam::new("Lamp1 X", 0.0, FloatRange::Linear { min: 0.0, max: 1.0 })
                .with_smoother(SmoothingStyle::Linear(5.0)),
            l3_val: FloatParam::new(
                "Lamp1 Value",
                0.0,
                FloatRange::Linear { min: 0.0, max: 1.0 },
            )
            .with_smoother(SmoothingStyle::Linear(5.0)),
            l3_clr: FloatParam::new(
                "Lamp1 Color",
                0.0,
                FloatRange::Linear { min: 0.0, max: 1.0 },
            )
            .with_smoother(SmoothingStyle::Linear(5.0)),
            l3_x: FloatParam::new("Lamp1 X", 0.0, FloatRange::Linear { min: 0.0, max: 1.0 })
                .with_smoother(SmoothingStyle::Linear(5.0)),
            l4_val: FloatParam::new(
                "Lamp1 Value",
                0.0,
                FloatRange::Linear { min: 0.0, max: 1.0 },
            )
            .with_smoother(SmoothingStyle::Linear(5.0)),
            l4_clr: FloatParam::new(
                "Lamp1 Color",
                0.0,
                FloatRange::Linear { min: 0.0, max: 1.0 },
            )
            .with_smoother(SmoothingStyle::Linear(5.0)),
            l4_x: FloatParam::new("Lamp1 X", 0.0, FloatRange::Linear { min: 0.0, max: 1.0 })
                .with_smoother(SmoothingStyle::Linear(5.0)),
            l5_val: FloatParam::new(
                "Lamp1 Value",
                0.0,
                FloatRange::Linear { min: 0.0, max: 1.0 },
            )
            .with_smoother(SmoothingStyle::Linear(5.0)),
            l5_clr: FloatParam::new(
                "Lamp1 Color",
                0.0,
                FloatRange::Linear { min: 0.0, max: 1.0 },
            )
            .with_smoother(SmoothingStyle::Linear(5.0)),
            l5_x: FloatParam::new("Lamp1 X", 0.0, FloatRange::Linear { min: 0.0, max: 1.0 })
                .with_smoother(SmoothingStyle::Linear(5.0)),

            wlambda_code: Arc::new(Mutex::new(String::from("!(note, is_on) = @;\n"))),
            wlambda_path: Arc::new(Mutex::new(String::from("/home/weicon/midi2wlambda.wl"))),
            wlambda_init_code: Arc::new(Mutex::new(String::from(""))),
            wlambda_init_path: Arc::new(Mutex::new(String::from(
                "/home/weicon/midi2wlambda_init.wl",
            ))),
        }
    }
}

pub enum M2WTask {
    MIDI(i64, bool),
    UpdateCode(String),
    InitCode(String, String),
}

struct GUIState {
    log: Vec<String>,
}

impl GUIState {
    fn new() -> Self {
        Self { log: vec![] }
    }
}

impl Midi2WLambda {
    fn start_wlambda_executor(&mut self) {
        let wl_handle = RPCHandle::new();
        let wl_handle_stopper = wl_handle.make_stopper_handle();
        self.wl_handle = wl_handle;
        self.wl_handle_stopper = Some(wl_handle_stopper);

        let (mut prod, mut cons) = RingBuffer::new(1024);
        self.task_queue = Some(prod);
        let (mut prod2, mut cons2) = RingBuffer::new(1024);
        if let Ok(mut q) = self.task_queue2.lock() {
            let _ = std::mem::replace(&mut *q, Some(prod2));
        }
        eprintln!("REPLACE");

        let handle = self.wl_handle.clone();
        let log = self.gui_log_channel.clone();
        let param_vec = self.param_vval_vec.clone();

        std::thread::spawn(move || {
            eprintln!("started bg thread");
            let mut wlctx = EvalContext::new_default();
            handle.register_global_functions("worker", &mut wlctx);
            let log2 = log.clone();
            let log3 = log.clone();
            wlctx.set_global_var(
                "log",
                &VValFun::new_fun(
                    move |env: &mut Env, _argc: usize| {
                        log.send(&env.arg(0));
                        Ok(VVal::None)
                    },
                    Some(1),
                    Some(1),
                    false,
                ),
            );

            wlctx.set_global_var("params", &VVal::Usr(Box::new(param_vec)));

            wlctx.set_global_var(
                "new_log_sender",
                &VValFun::new_fun(
                    move |env: &mut Env, _argc: usize| Ok(log2.fork_sender()),
                    Some(0),
                    Some(0),
                    false,
                ),
            );
            let _ = wlctx.eval("log :STARTUP_WLAMBDA");

            let rr = wlctx.eval(
                r#"
                !:global on_midi = {||};
                !:global update_midi_function = {!(code) = @;
                    !func = std:eval code;
                    .on_midi = unwrap func;
                };
            "#,
            );
            if let Err(e) = rr {
                eprintln!("RR: {}", e);
            }

            let mut rrr = 0;
            wlambda::rpc_helper::rpc_handler_cb(
                &mut wlctx,
                &handle,
                std::time::Duration::from_millis(10),
                move |wlctx| loop {
                    rrr += 1;
                    if rrr % 100 == 0 {
                        eprintln!("LOOP RUNNING {:?}", std::thread::current().id());
                    };

                    let task = if let Ok(task) = cons.pop() {
                        task
                    } else if let Ok(task) = cons2.pop() {
                        task
                    } else {
                        break;
                    };

                    match task {
                        M2WTask::MIDI(x, o) => {
                            if let Some(on_midi) = wlctx.get_global_var("on_midi") {
                                let _ = wlctx.call(&on_midi, &[VVal::Int(x as i64), VVal::Bol(o)]);
                            }
                        }
                        M2WTask::InitCode(init_code, midi_code) => {
                            eprintln!("UPDATE CODE!");
                            let r = wlctx.eval(&init_code);
                            log3.send(&VVal::new_str_mv(format!("Initialized! {:?}", r)));

                            if let Some(update_midi_function) =
                                wlctx.get_global_var("update_midi_function")
                            {
                                let r = wlctx.call(
                                    &update_midi_function,
                                    &[VVal::new_str(&midi_code)],
                                );
                                log3.send(&VVal::new_str_mv(format!(
                                    "Updated MIDI Function! {:?}",
                                    r
                                )));
                            }
                        }
                        M2WTask::UpdateCode(code) => {
                            eprintln!("UPDATE CODE!");
                            if let Some(update_midi_function) =
                                wlctx.get_global_var("update_midi_function")
                            {
                                let r = wlctx.call(&update_midi_function, &[VVal::new_str(&code)]);

                                log3.send(&VVal::new_str_mv(format!(
                                    "Updated MIDI Function! {:?}",
                                    r
                                )));
                            }
                        }
                    };
                },
            );
            eprintln!("end bg thread");
        });
    }
}

impl Plugin for Midi2WLambda {
    const NAME: &'static str = "MIDI2WLambda";
    const VENDOR: &'static str = "Weird Plugins";
    const URL: &'static str = "http://m8geil.de/";
    const EMAIL: &'static str = "weirdconstructor@m8geil.de";

    const VERSION: &'static str = env!("CARGO_PKG_VERSION");

    const AUDIO_IO_LAYOUTS: &'static [AudioIOLayout] = &[
        AudioIOLayout {
            main_input_channels: NonZeroU32::new(2),
            main_output_channels: NonZeroU32::new(2),
            ..AudioIOLayout::const_default()
        },
        AudioIOLayout {
            main_input_channels: NonZeroU32::new(1),
            main_output_channels: NonZeroU32::new(1),
            ..AudioIOLayout::const_default()
        },
    ];

    const MIDI_INPUT: MidiConfig = MidiConfig::Basic;

    const SAMPLE_ACCURATE_AUTOMATION: bool = true;

    type SysExMessage = ();
    type BackgroundTask = ();

    fn params(&self) -> Arc<dyn Params> {
        self.params.clone()
    }

    fn editor(&mut self, _async_executor: AsyncExecutor<Self>) -> Option<Box<dyn Editor>> {
        let params = self.params.clone();
        let chan = self.gui_log_channel.clone();
        let mut queue2 = self.task_queue2.clone();
        eprintln!("CREATE ERDI");

        create_egui_editor(
            self.params.editor_state.clone(),
            GUIState::new(),
            |_, _| {},
            move |egui_ctx, setter, state| {
                let mut r = chan.try_recv();
                while r.is_some() {
                    if state.log.len() > 100 {
                        state.log.remove(0);
                    }
                    state.log.push(r.s_raw());
                    r = chan.try_recv();
                }

                egui::CentralPanel::default().show(egui_ctx, |ui| {
                    // NOTE: See `plugins/diopser/src/editor.rs` for an example using the generic UI widget
                    ui.vertical(|ui| {
                        if let Ok(mut path) = params.wlambda_path.lock() {
                            if let Ok(mut init_path) = params.wlambda_init_path.lock() {
                                if ui.button("Update Init").clicked() {
                                    if let Ok(mut midi_code) = params.wlambda_code.lock() {
                                        if let Ok(mut init_code) = params.wlambda_init_code.lock() {
                                            if let Ok(m_code) = std::fs::read_to_string(&*path) {
                                                *midi_code = m_code;
                                            }

                                            if let Ok(txt) = std::fs::read_to_string(&*init_path) {
                                                *init_code = txt;
                                                if let Ok(mut q) = queue2.lock() {
                                                    if let Some(q) = q.as_mut() {
                                                        println!("PUSHTASK");
                                                        let _ = q.push(M2WTask::InitCode(
                                                            init_code.clone(),
                                                            midi_code.clone(),
                                                        ));
                                                    }
                                                }
                                            }
                                        }
                                    }
                                }

                                ui.text_edit_singleline(&mut *init_path);
                            }
                        }

                        if let Ok(mut path) = params.wlambda_path.lock() {
                            if ui.button("Update Code").clicked() {
                                if let Ok(mut code) = params.wlambda_code.lock() {
                                    if let Ok(txt) = std::fs::read_to_string(&*path) {
                                        *code = txt;
                                        if let Ok(mut q) = queue2.lock() {
                                            if let Some(q) = q.as_mut() {
                                                let _ = q.push(M2WTask::UpdateCode(code.clone()));
                                            }
                                        }
                                    }
                                }
                            }

                            ui.text_edit_singleline(&mut *path);
                        }

                        ui.columns(3, |columns| {
                            if let Ok(mut code) = params.wlambda_init_code.lock() {
                                egui::ScrollArea::vertical().id_source("init").show(
                                    &mut columns[0],
                                    |ui| {
                                        ui.add_sized(
                                            ui.available_size() * egui::Vec2::new(0.3, 1.0),
                                            egui::TextEdit::multiline(&mut *code)
                                                .code_editor()
                                                .desired_width(40.0)
                                                .desired_rows(29),
                                        );
                                    },
                                );
                            }

                            if let Ok(mut code) = params.wlambda_code.lock() {
                                egui::ScrollArea::vertical().id_source("code").show(
                                    &mut columns[1],
                                    |ui| {
                                        ui.add_sized(
                                            ui.available_size() * egui::Vec2::new(0.3, 1.0),
                                            egui::TextEdit::multiline(&mut *code)
                                                .code_editor()
                                                .desired_width(40.0)
                                                .desired_rows(29),
                                        );
                                    },
                                );
                            }

                            let log_str = state.log.join("\n");
                            let mut llstr: &str = &log_str;

                            egui::ScrollArea::vertical()
                                .id_source("log")
                                .stick_to_bottom(true)
                                .show(&mut columns[2], |ui| {
                                    ui.add_sized(
                                        ui.available_size(),
                                        egui::TextEdit::multiline(&mut llstr)
                                            .code_editor()
                                            .desired_width(40.0)
                                            .desired_rows(29),
                                    );
                                });
                        });
                    })

                    //                    // This is a fancy widget that can get all the information it needs to properly
                    //                    // display and modify the parameter from the parametr itself
                    //                    // It's not yet fully implemented, as the text is missing.
                    //                    ui.label("Some random integer");
                    //                    ui.add(widgets::ParamSlider::for_param(&params.some_int, setter));
                    //
                    //                    if params.some_int.value() == 1 {
                    //                        async_executor
                    //                            .execute_background(M2WTask::UpdateCode("test123".to_string()));
                    //                    }
                    //
                    //                    ui.label("Gain");
                    //                    ui.add(widgets::ParamSlider::for_param(&params.gain, setter));
                    //
                    //                    ui.label(
                    //                        "Also gain, but with a lame widget. Can't even render the value correctly!",
                    //                    );
                    //                    // This is a simple naieve version of a parameter slider that's not aware of how
                    //                    // the parameters work
                    //                    ui.add(
                    //                        egui::widgets::Slider::from_get_set(-30.0..=30.0, |new_value| {
                    //                            match new_value {
                    //                                Some(new_value_db) => {
                    //                                    let new_value = util::gain_to_db(new_value_db as f32);
                    //
                    //                                    setter.begin_set_parameter(&params.gain);
                    //                                    setter.set_parameter(&params.gain, new_value);
                    //                                    setter.end_set_parameter(&params.gain);
                    //
                    //                                    new_value_db
                    //                                }
                    //                                None => util::gain_to_db(params.gain.value()) as f64,
                    //                            }
                    //                        })
                    //                        .suffix(" dB"),
                    //                    );
                });
            },
        )
    }

    fn initialize(
        &mut self,
        _audio_io_layout: &AudioIOLayout,
        _buffer_config: &BufferConfig,
        _context: &mut impl InitContext<Self>,
    ) -> bool {
        true
    }

    fn reset(&mut self) {
        self.start_wlambda_executor();
    }

    fn process(
        &mut self,
        _buffer: &mut Buffer,
        _aux: &mut AuxiliaryBuffers,
        context: &mut impl ProcessContext<Self>,
    ) -> ProcessStatus {
        let mut next_event = context.next_event();
        let params = self.params.clone();
        self.param_vval_vec.set(0, params.l1_val.value());
        self.param_vval_vec.set(1, params.l1_clr.value());
        self.param_vval_vec.set(2, params.l1_x.value());

        self.param_vval_vec.set(3, params.l2_val.value());
        self.param_vval_vec.set(4, params.l2_clr.value());
        self.param_vval_vec.set(5, params.l2_x.value());

        self.param_vval_vec.set(6, params.l3_val.value());
        self.param_vval_vec.set(7, params.l3_clr.value());
        self.param_vval_vec.set(8, params.l3_x.value());

        self.param_vval_vec.set(9, params.l4_val.value());
        self.param_vval_vec.set(10, params.l4_clr.value());
        self.param_vval_vec.set(11, params.l4_x.value());

        self.param_vval_vec.set(12, params.l5_val.value());
        self.param_vval_vec.set(13, params.l5_clr.value());
        self.param_vval_vec.set(14, params.l5_x.value());

        while let Some(event) = next_event {
            match event {
                NoteEvent::NoteOn { note, velocity, .. } => {
                    if let Some(q) = self.task_queue.as_mut() {
                        let _ = q.push(M2WTask::MIDI(note as i64, true));
                    }
                }
                NoteEvent::NoteOff { note, .. } => {
                    if let Some(q) = self.task_queue.as_mut() {
                        let _ = q.push(M2WTask::MIDI(note as i64, false));
                    }
                }
                NoteEvent::PolyVolume { note, gain, .. } => {}
                _ => (),
            }

            next_event = context.next_event();
        }

        ProcessStatus::Normal
    }
}

impl ClapPlugin for Midi2WLambda {
    const CLAP_ID: &'static str = "de.m8geil.midi_to_wlambda";
    const CLAP_DESCRIPTION: Option<&'static str> = Some(
        "A plugin to route audio events and parameters to a WLambda script for controlling LEDs.",
    );
    const CLAP_MANUAL_URL: Option<&'static str> = Some(Self::URL);
    const CLAP_SUPPORT_URL: Option<&'static str> = None;
    const CLAP_FEATURES: &'static [ClapFeature] = &[ClapFeature::Instrument, ClapFeature::Utility];
}

impl Vst3Plugin for Midi2WLambda {
    const VST3_CLASS_ID: [u8; 16] = *b"MIDI2WLambdaAAAA";
    const VST3_SUBCATEGORIES: &'static [Vst3SubCategory] = &[
        Vst3SubCategory::Fx,
        Vst3SubCategory::Instrument,
        Vst3SubCategory::Tools,
    ];
}

nih_export_clap!(Midi2WLambda);
nih_export_vst3!(Midi2WLambda);
