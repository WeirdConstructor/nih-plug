use atomic_float::AtomicF32;
use nih_plug::prelude::*;
use nih_plug_egui::{create_egui_editor, egui, widgets, EguiState};
use std::sync::Arc;
use std::sync::Mutex;
use wlambda::rpc_helper::{RPCHandle, RPCHandleStopper};
use wlambda::threads::AValChannel;
use wlambda::vval::VValFun;
use wlambda::{Env, EvalContext, VVal};

/// This is mostly identical to the gain example, minus some fluff, and with a GUI.
#[allow(dead_code)]
pub struct Midi2WLambda {
    params: Arc<Midi2WLambdaParams>,

    wl_handle: RPCHandle,
    wl_handle_stopper: RPCHandleStopper,

    gui_log_channel: AValChannel,
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
}

impl Default for Midi2WLambda {
    fn default() -> Self {
        let wl_handle = RPCHandle::new();
        let wl_handle_stopper = wl_handle.make_stopper_handle();
        Self {
            params: Arc::new(Midi2WLambdaParams::default()),

            gui_log_channel: AValChannel::new_direct(),
            wl_handle,
            wl_handle_stopper,
        }
    }
}

impl Default for Midi2WLambdaParams {
    fn default() -> Self {
        Self {
            editor_state: EguiState::from_size(1200, 500),
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
        Self {
            log: vec![],
        }
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
    type BackgroundTask = M2WTask;

    fn params(&self) -> Arc<dyn Params> {
        self.params.clone()
    }

    fn task_executor(&mut self) -> TaskExecutor<Self> {
        let handle = self.wl_handle.clone();
        let log = self.gui_log_channel.clone();

        let comm_chan = AValChannel::new_direct();
        let recv = comm_chan.clone();

        std::thread::spawn(move || {
            let mut wlctx = EvalContext::new_default();
            handle.register_global_functions("worker", &mut wlctx);
            let log2 = log.clone();
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

            wlambda::rpc_helper::rpc_handler(
                &mut wlctx,
                &handle,
                std::time::Duration::from_millis(100),
            );
        });

        let sender = self.wl_handle.clone();
        let log = self.gui_log_channel.clone();

        Box::new(move |bg: M2WTask| match bg {
            M2WTask::MIDI(x, o) => {
                eprintln!("MIDI {} {}", x, o);
                let _ = sender.call("on_midi", VVal::vec2(VVal::Int(x as i64), VVal::Bol(o)));
            }
            M2WTask::InitCode(init_code, midi_code) => {
                let r = sender.eval(&init_code);
                log.send(&VVal::new_str_mv(format!("Initialized! {}", r.s())));

                let r = sender.call(
                    "update_midi_function",
                    VVal::vec1(VVal::new_str(&midi_code)),
                );
                log.send(&VVal::new_str_mv(format!(
                    "Updated MIDI Function! {}",
                    r.s()
                )));
            }
            M2WTask::UpdateCode(code) => {
                let r = sender.call("update_midi_function", VVal::vec1(VVal::new_str(&code)));
                log.send(&VVal::new_str_mv(format!(
                    "Updated MIDI Function! {}",
                    r.s()
                )));
            }
        })
    }

    fn editor(&mut self, async_executor: AsyncExecutor<Self>) -> Option<Box<dyn Editor>> {
        let params = self.params.clone();
        let chan = self.gui_log_channel.clone();

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

                                                async_executor.execute_background(
                                                    M2WTask::InitCode(
                                                        init_code.clone(),
                                                        midi_code.clone(),
                                                    ),
                                                );
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
                                        async_executor
                                            .execute_background(M2WTask::UpdateCode(code.clone()));
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
                            let mut llstr : &str = &log_str;

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
        buffer_config: &BufferConfig,
        _context: &mut impl InitContext<Self>,
    ) -> bool {
        true
    }

    fn process(
        &mut self,
        _buffer: &mut Buffer,
        _aux: &mut AuxiliaryBuffers,
        context: &mut impl ProcessContext<Self>,
    ) -> ProcessStatus {
        let mut next_event = context.next_event();

        while let Some(event) = next_event {
            match event {
                NoteEvent::NoteOn { note, velocity, .. } => {
                    context.execute_background(M2WTask::MIDI(note as i64, true));
                }
                NoteEvent::NoteOff { note, .. } => {
                    context.execute_background(M2WTask::MIDI(note as i64, false));
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
