use super::*;

#[pyclass]
pub(crate) struct AudioBuffer(pub(crate) web_audio_api_rs::AudioBuffer);

#[pymethods]
impl AudioBuffer {
    #[new]
    pub(crate) fn new(options: &Bound<'_, PyAny>) -> PyResult<Self> {
        Ok(Self(web_audio_api_rs::AudioBuffer::new(
            audio_buffer_options(options)?,
        )))
    }

    #[getter(numberOfChannels)]
    pub(crate) fn number_of_channels(&self) -> usize {
        self.0.number_of_channels()
    }

    #[getter]
    pub(crate) fn length(&self) -> usize {
        self.0.length()
    }

    #[getter(sampleRate)]
    pub(crate) fn sample_rate(&self) -> f32 {
        self.0.sample_rate()
    }

    #[getter]
    pub(crate) fn duration(&self) -> f64 {
        self.0.duration()
    }

    #[pyo3(name = "getChannelData")]
    pub(crate) fn get_channel_data(&self, channel_number: usize) -> PyResult<Vec<f32>> {
        catch_web_audio_panic_result(|| self.0.get_channel_data(channel_number).to_vec())
    }

    #[pyo3(name = "copyFromChannel", signature = (destination, channel_number, buffer_offset=0))]
    pub(crate) fn copy_from_channel(
        &self,
        mut destination: Vec<f32>,
        channel_number: usize,
        buffer_offset: usize,
    ) -> PyResult<Vec<f32>> {
        catch_web_audio_panic(|| {
            self.0
                .copy_from_channel_with_offset(&mut destination, channel_number, buffer_offset);
        })?;
        Ok(destination)
    }

    #[pyo3(name = "copyToChannel", signature = (source, channel_number, buffer_offset=0))]
    pub(crate) fn copy_to_channel(
        &mut self,
        source: Vec<f32>,
        channel_number: usize,
        buffer_offset: usize,
    ) -> PyResult<()> {
        catch_web_audio_panic(|| {
            self.0
                .copy_to_channel_with_offset(&source, channel_number, buffer_offset);
        })
    }
}

pub(crate) fn audio_buffer_options(
    options: &Bound<'_, PyAny>,
) -> PyResult<web_audio_api_rs::AudioBufferOptions> {
    let options = options
        .cast::<PyDict>()
        .map_err(|_| pyo3::exceptions::PyTypeError::new_err("AudioBufferOptions must be a dict"))?;

    let number_of_channels = options
        .get_item("numberOfChannels")?
        .map(|value| value.extract())
        .transpose()?
        .unwrap_or(1);
    let length = options
        .get_item("length")?
        .ok_or_else(|| {
            pyo3::exceptions::PyTypeError::new_err("AudioBufferOptions.length is required")
        })?
        .extract()?;
    let sample_rate = options
        .get_item("sampleRate")?
        .ok_or_else(|| {
            pyo3::exceptions::PyTypeError::new_err("AudioBufferOptions.sampleRate is required")
        })?
        .extract()?;

    Ok(web_audio_api_rs::AudioBufferOptions {
        number_of_channels,
        length,
        sample_rate,
    })
}
