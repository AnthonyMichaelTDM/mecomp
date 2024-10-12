//! Principal Component Analysis
//!
//! Principal Component Analysis is a common technique for data and dimensionality reduction. It
//! reduces the dimensionality of the data while retaining most of the variance. This is
//! done by projecting the data to a lower dimensional space with SVD and eigenvalue analysis. This
//! implementation uses the `TruncatedSvd` routine in `ndarray-linalg` which employs LOBPCG.
//!
//! # Example
//!
//! ```
//! use linfa::traits::{Fit, Predict};
//! use linfa_reduction::Pca;
//!
//! let dataset = linfa_datasets::iris();
//!
//! // apply PCA projection along a line which maximizes the spread of the data
//! let embedding = Pca::params(1)
//!     .fit(&dataset).unwrap();
//!
//! // reduce dimensionality of the dataset
//! let dataset = embedding.predict(dataset);
//! ```
//!
use linfa_linalg::{lobpcg::TruncatedSvd, Order};
use ndarray::{Array1, Array2, ArrayBase, Axis, Data, Ix2};
use rand::{prelude::SmallRng, SeedableRng};

use linfa::{
    dataset::Records,
    traits::{Fit, PredictInplace, Transformer},
    DatasetBase, Float,
};

use thiserror::Error;

pub type Result<T> = std::result::Result<T, ReductionError>;

#[derive(Error, Debug)]
#[non_exhaustive]
pub enum ReductionError {
    #[error("At least 1 sample needed")]
    NotEnoughSamples,
    #[error("embedding dimension smaller {0} than feature dimension")]
    EmbeddingTooSmall(usize),
    #[error(transparent)]
    LinalgError(#[from] linfa_linalg::LinalgError),
    #[error(transparent)]
    LinfaError(#[from] linfa::error::Error),
    #[error(transparent)]
    NdarrayRandError(#[from] ndarray_rand::rand_distr::NormalError),
}

/// Pincipal Component Analysis parameters
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PcaParams {
    embedding_size: usize,
    apply_whitening: bool,
}

impl PcaParams {
    /// Apply whitening to the embedding vector
    ///
    /// Whitening will scale the eigenvalues of the transformation such that the covariance will be
    /// unit diagonal for the original data.
    pub fn whiten(mut self, apply: bool) -> Self {
        self.apply_whitening = apply;

        self
    }
}

/// Fit a PCA model given a dataset
///
/// The Principal Component Analysis takes the records of a dataset and tries to find the best
/// fit in a lower dimensional space such that the maximal variance is retained.
///
/// # Parameters
///
/// * `dataset`: A dataset with records in N dimensions
///
/// # Returns
///
/// A fitted PCA model with origin and hyperplane
impl<T, D: Data<Elem = f64>> Fit<ArrayBase<D, Ix2>, T, ReductionError> for PcaParams {
    type Object = Pca<f64>;

    fn fit(&self, dataset: &DatasetBase<ArrayBase<D, Ix2>, T>) -> Result<Pca<f64>> {
        if dataset.nsamples() == 0 {
            return Err(ReductionError::NotEnoughSamples);
        } else if dataset.nfeatures() < self.embedding_size || self.embedding_size == 0 {
            return Err(ReductionError::EmbeddingTooSmall(self.embedding_size));
        }

        let x = dataset.records();
        // calculate mean of data and subtract it
        // safe because of above 0 samples check
        let mean = x.mean_axis(Axis(0)).unwrap();
        let x = x - &mean;

        // estimate Singular Value Decomposition
        let result = TruncatedSvd::new_with_rng(x, Order::Largest, SmallRng::seed_from_u64(42))
            .decompose(self.embedding_size)?;
        // explained variance is the spectral distribution of the eigenvalues
        let (_, sigma, mut v_t) = result.values_vectors();

        // cut singular values to avoid numerical problems
        let sigma = sigma.mapv(|x| x.max(1e-8));

        // scale the embedding with the square root of the dimensionality and eigenvalue such that
        // the product of the resulting matrix gives the unit covariance.
        if self.apply_whitening {
            let cov_scale = (dataset.nsamples() as f64 - 1.).sqrt();
            for (mut v_t, sigma) in v_t.axis_iter_mut(Axis(0)).zip(sigma.iter()) {
                v_t *= cov_scale / *sigma;
            }
        }

        Ok(Pca {
            embedding: v_t,
            sigma,
            mean,
        })
    }
}

/// Fitted Principal Component Analysis model
///
/// The model contains the mean and hyperplane for the projection of data.
///
/// # Example
///
/// ```
/// use linfa::traits::{Fit, Predict};
/// use linfa_reduction::Pca;
///
/// let dataset = linfa_datasets::iris();
///
/// // apply PCA projection along a line which maximizes the spread of the data
/// let embedding = Pca::params(1)
///     .fit(&dataset).unwrap();
///
/// // reduce dimensionality of the dataset
/// let dataset = embedding.predict(dataset);
/// ```
#[derive(Debug, Clone, PartialEq)]
pub struct Pca<F> {
    embedding: Array2<F>,
    sigma: Array1<F>,
    mean: Array1<F>,
}

impl Pca<f64> {
    /// Create default parameter set
    ///
    /// # Parameters
    ///
    ///  * `embedding_size`: the target dimensionality
    pub const fn params(embedding_size: usize) -> PcaParams {
        PcaParams {
            embedding_size,
            apply_whitening: false,
        }
    }

    // /// Return the amount of explained variance per element
    // pub fn explained_variance(&self) -> Array1<f64> {
    //     self.sigma.mapv(|x| x * x / (self.sigma.len() as f64 - 1.0))
    // }

    // /// Return the normalized amount of explained variance per element
    // pub fn explained_variance_ratio(&self) -> Array1<f64> {
    //     let ex_var = self.sigma.mapv(|x| x * x / (self.sigma.len() as f64 - 1.0));
    //     let sum_ex_var = ex_var.sum();

    //     ex_var / sum_ex_var
    // }

    // /// Return the components
    // pub fn components(&self) -> &Array2<f64> {
    //     &self.embedding
    // }

    // /// Return the mean
    // pub fn mean(&self) -> &Array1<f64> {
    //     &self.mean
    // }

    // /// Return the singular values
    // pub fn singular_values(&self) -> &Array1<f64> {
    //     &self.sigma
    // }

    // /// Transform data back to its original space
    // pub fn inverse_transform(
    //     &self,
    //     prediction: ArrayBase<ndarray::OwnedRepr<f64>, ndarray::Dim<[usize; 2]>>,
    // ) -> ArrayBase<ndarray::OwnedRepr<f64>, ndarray::Dim<[usize; 2]>> {
    //     prediction.dot(&self.embedding) + &self.mean
    // }
}

impl<F: Float, D: Data<Elem = F>> PredictInplace<ArrayBase<D, Ix2>, Array2<F>> for Pca<F> {
    fn predict_inplace(&self, records: &ArrayBase<D, Ix2>, targets: &mut Array2<F>) {
        assert_eq!(
            targets.shape(),
            &[records.nrows(), self.embedding.nrows()],
            "The number of data points must match the number of output targets."
        );
        *targets = (records - &self.mean).dot(&self.embedding.t());
    }

    fn default_target(&self, x: &ArrayBase<D, Ix2>) -> Array2<F> {
        Array2::zeros((x.nrows(), self.embedding.nrows()))
    }
}

impl<F: Float, D: Data<Elem = F>, T>
    Transformer<DatasetBase<ArrayBase<D, Ix2>, T>, DatasetBase<Array2<F>, T>> for Pca<F>
{
    fn transform(&self, ds: DatasetBase<ArrayBase<D, Ix2>, T>) -> DatasetBase<Array2<F>, T> {
        let DatasetBase {
            records,
            targets,
            weights,
            ..
        } = ds;

        let mut new_records = self.default_target(&records);
        self.predict_inplace(&records, &mut new_records);

        DatasetBase::new(new_records, targets).with_weights(weights)
    }
}
