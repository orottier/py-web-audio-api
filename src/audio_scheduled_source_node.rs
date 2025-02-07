use web_audio_api::node::{self, AudioNode, AudioScheduledSourceNode, ChannelConfig};
use web_audio_api::context::AudioContextRegistration;

pub(crate) enum ConcreteAudioScheduledSourceNode {
    Buffer(Arc<Mutex<node::AudioBufferSourceNode>>),
    Constant(Arc<Mutex<node::ConstantSourceNode>>),
    Oscillator(Arc<Mutex<node::OscillatorNode>>),
}

use ConcreteAudioScheduledSourceNode::*;

impl AudioNode for ConcreteAudioScheduledSourceNode {
    fn registration(&self) -> &AudioContextRegistration {
        match self {
            Buffer(n) => n.registration(),
            Constant(n) => n.registration(),
            Oscillator(n) => n.registration(),
        }
    }

    fn channel_config(&self) -> &ChannelConfig {
        match self {
            Buffer(n) => n.channel_config(),
            Constant(n) => n.channel_config(),
            Oscillator(n) => n.channel_config(),
        }
    }

    fn number_of_inputs(&self) -> usize {
        match self {
            Buffer(n) => n.number_of_inputs(),
            Constant(n) => n.number_of_inputs(),
            Oscillator(n) => n.number_of_inputs(),
        }
    }

    fn number_of_outputs(&self) -> usize {
        match self {
            Buffer(n) => n.number_of_outputs(),
            Constant(n) => n.number_of_outputs(),
            Oscillator(n) => n.number_of_outputs(),
        }
    }
}

impl AudioScheduledSourceNode for ConcreteAudioScheduledSourceNode {
    fn start(&mut self) {
        match self {
            Buffer(n) => n.start(),
            Constant(n) => n.start(),
            Oscillator(n) => n.start(),
        }
    }

    fn start_at(&mut self, when: f64) {
        match self {
            Buffer(n) => n.start_at(when),
            Constant(n) => n.start_at(when),
            Oscillator(n) => n.start_at(when),
        }
    }

    fn stop(&mut self) {
        match self {
            Buffer(n) => n.stop(),
            Constant(n) => n.stop(),
            Oscillator(n) => n.stop(),
        }
    }

    fn stop_at(&mut self, when: f64) {
        match self {
            Buffer(n) => n.stop_at(when),
            Constant(n) => n.stop_at(when),
            Oscillator(n) => n.stop_at(when),
        }
    }
}
