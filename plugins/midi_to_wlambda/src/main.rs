use nih_plug::prelude::*;

use midi_to_wlambda::Midi2WLambda;

fn main() {
    nih_export_standalone::<Midi2WLambda>();
}
