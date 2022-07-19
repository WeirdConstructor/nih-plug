//! Features a plugin supports. This is essentially the same thing as tags, keyword, or categories.
//! Hosts may use these to organize plugins.

/// A keyword for a CLAP plugin. See
/// <https://github.com/free-audio/clap/blob/main/include/clap/plugin-features.h> for more
/// information.
pub enum ClapFeature {
    Instrument,
    AudioEffect,
    NoteEffect,
    Analyzer,
    Synthesizer,
    Sampler,
    Drum,
    DrumMachine,
    Filter,
    Phaser,
    Equalizer,
    Deesser,
    PhaseVocoder,
    Granular,
    FrequencyShifter,
    PitchShifter,
    Distortion,
    TransientShaper,
    Compressor,
    Limiter,
    Flanger,
    Chorus,
    Delay,
    Reverb,
    Tremolo,
    Glitch,
    Utility,
    PitchCorrection,
    Restoration,
    MultiEffects,
    Mixing,
    Mastering,
    Mono,
    Stereo,
    Surround,
    Ambisonic,
    /// A non-predefined feature. Hosts may display this among its plugin categories.
    Custom(&'static str),
}

impl ClapFeature {
    pub fn as_str(&self) -> &'static str {
        match self {
            ClapFeature::Instrument => "instrument",
            ClapFeature::AudioEffect => "audio-effect",
            ClapFeature::NoteEffect => "note-effect",
            ClapFeature::Analyzer => "analyzer",
            ClapFeature::Synthesizer => "synthesizer",
            ClapFeature::Sampler => "sampler",
            ClapFeature::Drum => "drum",
            ClapFeature::DrumMachine => "drum-machine",
            ClapFeature::Filter => "filter",
            ClapFeature::Phaser => "phaser",
            ClapFeature::Equalizer => "equalizer",
            ClapFeature::Deesser => "de-esser",
            ClapFeature::PhaseVocoder => "phase-vocoder",
            ClapFeature::Granular => "granular",
            ClapFeature::FrequencyShifter => "frequency-shifter",
            ClapFeature::PitchShifter => "pitch-shifter",
            ClapFeature::Distortion => "distortion",
            ClapFeature::TransientShaper => "transient-shaper",
            ClapFeature::Compressor => "compressor",
            ClapFeature::Limiter => "limiter",
            ClapFeature::Flanger => "flanger",
            ClapFeature::Chorus => "chorus",
            ClapFeature::Delay => "delay",
            ClapFeature::Reverb => "reverb",
            ClapFeature::Tremolo => "tremolo",
            ClapFeature::Glitch => "glitch",
            ClapFeature::Utility => "utility",
            ClapFeature::PitchCorrection => "pitch-correction",
            ClapFeature::Restoration => "restoration",
            ClapFeature::MultiEffects => "multi-effects",
            ClapFeature::Mixing => "mixing",
            ClapFeature::Mastering => "mastering",
            ClapFeature::Mono => "mono",
            ClapFeature::Stereo => "stereo",
            ClapFeature::Surround => "surround",
            ClapFeature::Ambisonic => "ambisonic",
            ClapFeature::Custom(s) => s,
        }
    }
}
