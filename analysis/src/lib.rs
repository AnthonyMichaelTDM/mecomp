//! This library contains stuff for song analysis and feature extraction.
//!
//! A lot of the code in this library is inspired by, or directly pulled from, [bliss-rs](https://github.com/Polochon-street/bliss-rs).
//! We don't simply use bliss-rs because I don't want to bring in an ffmpeg dependency, and bliss-rs also has a lot of features that I don't need.
//! (for example, I don't need to decode tags, process playlists, etc. etc., I'm doing all of that myself already)
//!
//! We use rodio to decode the audio file (overkill, but we already have the dependency for audio playback so may as well),
//! We use rubato to resample the audio file to 22050 Hz.

#![deny(clippy::missing_inline_in_public_items)]

pub mod chroma;
pub mod clustering;
pub mod decoder;
pub mod errors;
pub mod misc;
pub mod temporal;
pub mod timbral;
pub mod utils;

use std::{ops::Index, path::PathBuf};

use misc::LoudnessDesc;
use serde::{Deserialize, Serialize};
use strum::{EnumCount, EnumIter, IntoEnumIterator};

use chroma::ChromaDesc;
use errors::{AnalysisError, AnalysisResult};
use temporal::BPMDesc;
use timbral::{SpectralDesc, ZeroCrossingRateDesc};

/// The resampled audio data used for analysis.
///
/// Must be in mono (1 channel), with a sample rate of 22050 Hz.
#[derive(Debug)]
pub struct ResampledAudio {
    pub path: PathBuf,
    pub samples: Vec<f32>,
}

impl TryInto<Analysis> for ResampledAudio {
    type Error = AnalysisError;

    #[inline]
    fn try_into(self) -> Result<Analysis, Self::Error> {
        Analysis::from_samples(&self)
    }
}

/// The sampling rate used for the analysis.
pub const SAMPLE_RATE: u32 = 22050;

#[derive(Debug, EnumIter, EnumCount)]
/// Indexes different fields of an Analysis.
///
/// Prints the tempo value of an analysis.
///
/// Note that this should mostly be used for debugging / distance metric
/// customization purposes.
#[allow(missing_docs, clippy::module_name_repetitions)]
pub enum AnalysisIndex {
    Tempo,
    Zcr,
    MeanSpectralCentroid,
    StdDeviationSpectralCentroid,
    MeanSpectralRolloff,
    StdDeviationSpectralRolloff,
    MeanSpectralFlatness,
    StdDeviationSpectralFlatness,
    MeanLoudness,
    StdDeviationLoudness,
    Chroma1,
    Chroma2,
    Chroma3,
    Chroma4,
    Chroma5,
    Chroma6,
    Chroma7,
    Chroma8,
    Chroma9,
    Chroma10,
}

/// The Type of individual features
pub type Feature = f64;
/// The number of features used in `Analysis`
pub const NUMBER_FEATURES: usize = AnalysisIndex::COUNT;

#[derive(Default, PartialEq, Clone, Copy, Serialize, Deserialize)]
/// Object holding the results of the song's analysis.
///
/// Only use it if you want to have an in-depth look of what is
/// happening behind the scene, or make a distance metric yourself.
///
/// Under the hood, it is just an array of f32 holding different numeric
/// features.
///
/// For more info on the different features, build the
/// documentation with private items included using
/// `cargo doc --document-private-items`, and / or read up
/// [this document](https://lelele.io/thesis.pdf), that contains a description
/// on most of the features, except the chroma ones, which are documented
/// directly in this code.
pub struct Analysis {
    pub(crate) internal_analysis: [Feature; NUMBER_FEATURES],
}

impl Index<AnalysisIndex> for Analysis {
    type Output = Feature;

    #[inline]
    fn index(&self, index: AnalysisIndex) -> &Feature {
        &self.internal_analysis[index as usize]
    }
}

impl Index<usize> for Analysis {
    type Output = Feature;

    #[inline]
    fn index(&self, index: usize) -> &Feature {
        &self.internal_analysis[index]
    }
}

impl std::fmt::Debug for Analysis {
    #[inline]
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut debug_struct = f.debug_struct("Analysis");
        for feature in AnalysisIndex::iter() {
            debug_struct.field(&format!("{feature:?}"), &self[feature]);
        }
        debug_struct.finish()?;
        f.write_str(&format!(" /* {:?} */", &self.as_vec()))
    }
}

impl Analysis {
    /// Create a new Analysis object.
    ///
    /// Usually not needed, unless you have already computed and stored
    /// features somewhere, and need to recreate a Song with an already
    /// existing Analysis yourself.
    #[must_use]
    #[inline]
    pub const fn new(analysis: [Feature; NUMBER_FEATURES]) -> Self {
        Self {
            internal_analysis: analysis,
        }
    }

    /// Creates a new `Analysis` object from a `Vec<Feature>`.
    ///
    /// invariant: `features.len() == NUMBER_FEATURES`
    ///
    /// # Errors
    ///
    /// This function will return an error if the length of the features is not equal to `NUMBER_FEATURES`.
    #[inline]
    pub fn from_vec(features: Vec<Feature>) -> Result<Self, AnalysisError> {
        features
            .try_into()
            .map_err(|_| AnalysisError::InvalidFeaturesLen)
            .map(Self::new)
    }

    /// Return the inner array of the analysis.
    /// This is mostly useful if you want to store the features somewhere.
    #[must_use]
    #[inline]
    pub const fn inner(&self) -> &[Feature; NUMBER_FEATURES] {
        &self.internal_analysis
    }

    /// Return a `Vec<f32>` representing the analysis' features.
    ///
    /// Particularly useful if you want iterate through the values to store
    /// them somewhere.
    #[must_use]
    #[inline]
    pub fn as_vec(&self) -> Vec<Feature> {
        self.internal_analysis.to_vec()
    }

    /// Create an `Analysis` object from a `ResampledAudio`.
    /// This is the main function you should use to create an `Analysis` object.
    /// It will compute all the features from the audio samples.
    /// You can get a `ResampledAudio` object by using a `Decoder` to decode an audio file.
    ///
    /// # Errors
    ///
    /// This function will return an error if the samples are empty or too short.
    /// Or if there is an error during the analysis.
    ///
    /// # Panics
    ///
    /// This function will panic it cannot join the threads.
    #[allow(clippy::missing_inline_in_public_items)]
    pub fn from_samples(audio: &ResampledAudio) -> AnalysisResult<Self> {
        let largest_window = vec![
            BPMDesc::WINDOW_SIZE,
            ChromaDesc::WINDOW_SIZE,
            SpectralDesc::WINDOW_SIZE,
            LoudnessDesc::WINDOW_SIZE,
        ]
        .into_iter()
        .max()
        .unwrap();
        if audio.samples.len() < largest_window {
            return Err(AnalysisError::EmptySamples);
        }

        std::thread::scope(|s| -> AnalysisResult<Self> {
            let child_tempo: std::thread::ScopedJoinHandle<AnalysisResult<Feature>> =
                s.spawn(|| {
                    let mut tempo_desc = BPMDesc::new(SAMPLE_RATE)?;
                    let windows = audio
                        .samples
                        .windows(BPMDesc::WINDOW_SIZE)
                        .step_by(BPMDesc::HOP_SIZE);

                    for window in windows {
                        tempo_desc.do_(window)?;
                    }
                    Ok(tempo_desc.get_value())
                });

            let child_chroma: std::thread::ScopedJoinHandle<AnalysisResult<Vec<Feature>>> = s
                .spawn(|| {
                    let mut chroma_desc = ChromaDesc::new(SAMPLE_RATE, 12);
                    chroma_desc.do_(&audio.samples)?;
                    Ok(chroma_desc.get_value())
                });

            #[allow(clippy::type_complexity)]
            let child_timbral: std::thread::ScopedJoinHandle<
                AnalysisResult<(Vec<Feature>, Vec<Feature>, Vec<Feature>)>,
            > = s.spawn(|| {
                let mut spectral_desc = SpectralDesc::new(SAMPLE_RATE)?;
                let windows = audio
                    .samples
                    .windows(SpectralDesc::WINDOW_SIZE)
                    .step_by(SpectralDesc::HOP_SIZE);
                for window in windows {
                    spectral_desc.do_(window)?;
                }
                let centroid = spectral_desc.get_centroid();
                let rolloff = spectral_desc.get_rolloff();
                let flatness = spectral_desc.get_flatness();
                Ok((centroid, rolloff, flatness))
            });

            let child_zcr: std::thread::ScopedJoinHandle<AnalysisResult<Feature>> = s.spawn(|| {
                let mut zcr_desc = ZeroCrossingRateDesc::default();
                zcr_desc.do_(&audio.samples);
                Ok(zcr_desc.get_value())
            });

            let child_loudness: std::thread::ScopedJoinHandle<AnalysisResult<Vec<Feature>>> = s
                .spawn(|| {
                    let mut loudness_desc = LoudnessDesc::default();
                    let windows = audio.samples.chunks(LoudnessDesc::WINDOW_SIZE);

                    for window in windows {
                        loudness_desc.do_(window);
                    }
                    Ok(loudness_desc.get_value())
                });

            // Non-streaming approach for that one
            let tempo = child_tempo.join().unwrap()?;
            let chroma = child_chroma.join().unwrap()?;
            let (centroid, rolloff, flatness) = child_timbral.join().unwrap()?;
            let loudness = child_loudness.join().unwrap()?;
            let zcr = child_zcr.join().unwrap()?;

            let mut result = vec![tempo, zcr];
            result.extend_from_slice(&centroid);
            result.extend_from_slice(&rolloff);
            result.extend_from_slice(&flatness);
            result.extend_from_slice(&loudness);
            result.extend_from_slice(&chroma);
            let array: [Feature; NUMBER_FEATURES] = result
                .try_into()
                .map_err(|_| AnalysisError::InvalidFeaturesLen)?;
            Ok(Self::new(array))
        })
    }
}
