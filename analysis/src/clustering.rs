//! this module contains helpers that wrap the a k-means crate to perform clustering on the data
//! without having to choose an exact number of clusters.
//!
//! Instead, you provide the minimum and maximum number of clusters you want to try, and we'll
//! use one of a range of methods to determine the optimal number of clusters.
//!
//! # References:
//!
//! - The gap statistic [R. Tibshirani, G. Walther, and T. Hastie (Standford University, 2001)](https://hastie.su.domains/Papers/gap.pdf)
//! - The Davies-Bouldin index [wikipedia](https://en.wikipedia.org/wiki/Davies%E2%80%93Bouldin_index)

use linfa::prelude::*;
use linfa_clustering::{GaussianMixtureModel, KMeans};
use linfa_nn::distance::{Distance, L2Dist};
use linfa_reduction::Pca;
use linfa_tsne::TSneParams;
use log::{debug, info};
use ndarray::{Array, Array1, Array2, ArrayView1, ArrayView2, Axis, Dim};
use ndarray_rand::RandomExt;
use rand::distributions::Uniform;
use rayon::iter::{IntoParallelIterator, IntoParallelRefIterator, ParallelIterator};
use statrs::statistics::Statistics;

use crate::{
    Analysis, Feature, NUMBER_FEATURES,
    errors::{ClusteringError, ProjectionError},
};

pub struct AnalysisArray(pub(crate) Array2<Feature>);

impl From<Vec<Analysis>> for AnalysisArray {
    #[inline]
    fn from(data: Vec<Analysis>) -> Self {
        let shape = (data.len(), NUMBER_FEATURES);
        debug_assert_eq!(shape, (data.len(), data[0].inner().len()));

        Self(
            Array2::from_shape_vec(shape, data.into_iter().flat_map(|a| *a.inner()).collect())
                .expect("Failed to convert to array, shape mismatch"),
        )
    }
}

impl From<Vec<[Feature; NUMBER_FEATURES]>> for AnalysisArray {
    #[inline]
    fn from(data: Vec<[Feature; NUMBER_FEATURES]>) -> Self {
        let shape = (data.len(), NUMBER_FEATURES);
        debug_assert_eq!(shape, (data.len(), data[0].len()));

        Self(
            Array2::from_shape_vec(shape, data.into_iter().flatten().collect())
                .expect("Failed to convert to array, shape mismatch"),
        )
    }
}

pub type FitDataset = Dataset<Feature, (), Dim<[usize; 1]>>;

#[derive(Clone, Copy, Debug)]
#[allow(clippy::module_name_repetitions)]
pub enum ClusteringMethod {
    KMeans,
    GaussianMixtureModel,
}

impl ClusteringMethod {
    /// Fit the clustering method to the dataset and get the Labels
    #[must_use]
    fn fit(self, k: usize, data: &FitDataset) -> Array1<usize> {
        match self {
            Self::KMeans => {
                let model = KMeans::params(k)
                    // .max_n_iterations(MAX_ITERATIONS)
                    .fit(data)
                    .unwrap();
                model.predict(data.records())
            }
            Self::GaussianMixtureModel => {
                let model = GaussianMixtureModel::params(k)
                    .init_method(linfa_clustering::GmmInitMethod::KMeans)
                    .n_runs(10)
                    .fit(data)
                    .unwrap();
                model.predict(data.records())
            }
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub enum KOptimal {
    GapStatistic {
        /// The number of reference datasets to generate
        b: u32,
    },
    DaviesBouldin,
}

#[derive(Clone, Copy, Debug, Default)]
/// Should the data be projected into a lower-dimensional space before clustering, if so how?
pub enum ProjectionMethod {
    /// Use t-SNE to project the data into a lower-dimensional space
    TSne,
    /// Use PCA to project the data into a lower-dimensional space
    Pca,
    #[default]
    /// Don't project the data
    None,
}

impl ProjectionMethod {
    /// Project the data into a lower-dimensional space
    ///
    /// # Errors
    ///
    /// Will return an error if there was an error projecting the data into a lower-dimensional space
    #[inline]
    pub fn project(self, samples: AnalysisArray) -> Result<Array2<Feature>, ProjectionError> {
        let result = match self {
            Self::TSne => {
                let nrecords = samples.0.nrows();
                // first use the t-SNE algorithm to project the data into a lower-dimensional space
                debug!("Generating embeddings (size: {EMBEDDING_SIZE}) using t-SNE");
                #[allow(clippy::cast_precision_loss)]
                let mut embeddings = TSneParams::embedding_size(EMBEDDING_SIZE)
                    .perplexity(f64::max(samples.0.nrows() as f64 / 20., 5.))
                    .approx_threshold(0.5)
                    .transform(samples.0)?;
                debug_assert_eq!(embeddings.shape(), &[nrecords, EMBEDDING_SIZE]);

                // normalize the embeddings so each dimension is between -1 and 1
                debug!("Normalizing embeddings");
                normalize_embeddings_inplace::<EMBEDDING_SIZE>(&mut embeddings);
                embeddings
            }
            Self::Pca => {
                let nrecords = samples.0.nrows();
                // use the PCA algorithm to project the data into a lower-dimensional space
                debug!("Generating embeddings (size: {EMBEDDING_SIZE}) using PCA");
                let data = Dataset::from(samples.0);
                let pca: Pca<f64> = Pca::params(EMBEDDING_SIZE).whiten(true).fit(&data)?;
                let mut embeddings = pca.predict(&data);
                debug_assert_eq!(embeddings.shape(), &[nrecords, EMBEDDING_SIZE]);

                // normalize the embeddings so each dimension is between -1 and 1
                debug!("Normalizing embeddings");
                normalize_embeddings_inplace::<EMBEDDING_SIZE>(&mut embeddings);
                embeddings
            }
            Self::None => {
                debug!("Using original data as embeddings");
                samples.0
            }
        };
        debug!("Embeddings shape: {:?}", result.shape());
        Ok(result)
    }
}

// Normalize the embeddings to between 0.0 and 1.0, in-place.
// Pass the embedding size as an argument to enable more compiler optimizations
fn normalize_embeddings_inplace<const SIZE: usize>(embeddings: &mut Array2<f64>) {
    for i in 0..SIZE {
        let min = embeddings.column(i).min();
        let max = embeddings.column(i).max();
        let range = max - min;
        embeddings
            .column_mut(i)
            .mapv_inplace(|v| ((v - min) / range).mul_add(2., -1.));
    }
}

// log the number of features
/// Dimensionality that the T-SNE and PCA projection methods aim to project the data into.
const EMBEDDING_SIZE: usize = {
    let log2 = usize::ilog2(NUMBER_FEATURES) as usize;
    if log2 < 2 { 2 } else { log2 }
};

#[allow(clippy::module_name_repetitions)]
pub struct ClusteringHelper<S>
where
    S: Sized,
{
    state: S,
}

pub struct EntryPoint;
pub struct NotInitialized {
    /// The embeddings of our input, as a Nx`EMBEDDING_SIZE` array
    embeddings: Array2<Feature>,
    pub k_max: usize,
    pub optimizer: KOptimal,
    pub clustering_method: ClusteringMethod,
}
pub struct Initialized {
    /// The embeddings of our input, as a Nx`EMBEDDING_SIZE` array
    embeddings: Array2<Feature>,
    pub k: usize,
    pub clustering_method: ClusteringMethod,
}
pub struct Finished {
    /// The labelings of the samples, as a Nx1 array.
    /// Each element is the cluster that the corresponding sample belongs to.
    labels: Array1<usize>,
    pub k: usize,
}

/// Functions available for all states
impl ClusteringHelper<EntryPoint> {
    /// Create a new `KMeansHelper` object
    ///
    /// # Errors
    ///
    /// Will return an error if there was an error projecting the data into a lower-dimensional space
    #[allow(clippy::missing_inline_in_public_items)]
    pub fn new(
        samples: AnalysisArray,
        k_max: usize,
        optimizer: KOptimal,
        clustering_method: ClusteringMethod,
        projection_method: ProjectionMethod,
    ) -> Result<ClusteringHelper<NotInitialized>, ClusteringError> {
        if samples.0.nrows() <= 15 {
            return Err(ClusteringError::SmallLibrary);
        }

        // project the data into a lower-dimensional space
        let embeddings = projection_method.project(samples)?;

        Ok(ClusteringHelper {
            state: NotInitialized {
                embeddings,
                k_max,
                optimizer,
                clustering_method,
            },
        })
    }
}

/// Functions available for `NotInitialized` state
impl ClusteringHelper<NotInitialized> {
    /// Initialize the `KMeansHelper` object
    ///
    /// # Errors
    ///
    /// Will return an error if there was an error calculating the optimal number of clusters
    #[inline]
    pub fn initialize(self) -> Result<ClusteringHelper<Initialized>, ClusteringError> {
        let k = self.get_optimal_k()?;
        Ok(ClusteringHelper {
            state: Initialized {
                embeddings: self.state.embeddings,
                k,
                clustering_method: self.state.clustering_method,
            },
        })
    }

    fn get_optimal_k(&self) -> Result<usize, ClusteringError> {
        match self.state.optimizer {
            KOptimal::GapStatistic { b } => self.get_optimal_k_gap_statistic(b),
            KOptimal::DaviesBouldin => self.get_optimal_k_davies_bouldin(),
        }
    }

    /// Get the optimal number of clusters using the gap statistic
    ///
    /// # References:
    ///
    /// - [R. Tibshirani, G. Walther, and T. Hastie (Standford University, 2001)](https://hastie.su.domains/Papers/gap.pdf)
    ///
    /// # Algorithm:
    ///
    /// 1. Cluster the observed data, varying the number of clusters from k = 1, …, kmax, and compute the corresponding total within intra-cluster variation Wk.
    /// 2. Generate B reference data sets with a random uniform distribution. Cluster each of these reference data sets with varying number of clusters k = 1, …, kmax,
    ///    and compute the corresponding total within intra-cluster variation `W_{kb}`.
    /// 3. Compute the estimated gap statistic as the deviation of the observed `W_k` value from its expected value `W_kb` under the null hypothesis:
    ///    `Gap(k)=(1/B) \sum_{b=1}^{B} \log(W^*_{kb}) − \log(W_k)`.
    ///    Compute also the standard deviation of the statistics.
    /// 4. Choose the number of clusters as the smallest value of k such that the gap statistic is within one standard deviation of the gap at k+1:
    ///    `Gap(k)≥Gap(k + 1)−s_{k + 1}`.
    fn get_optimal_k_gap_statistic(&self, b: u32) -> Result<usize, ClusteringError> {
        let embedding_dataset = Dataset::from(self.state.embeddings.clone());

        // our reference data sets
        let reference_datasets =
            generate_reference_datasets(embedding_dataset.records().view(), b as usize);

        let b = f64::from(b);

        let results = (1..=self.state.k_max)
            // for each k, cluster the data into k clusters
            .map(|k| {
                debug!("Fitting k-means to embeddings with k={k}");
                let labels = self.state.clustering_method.fit(k, &embedding_dataset);
                (k, labels)
            })
            // for each k, calculate the gap statistic, and the standard deviation of the statistics
            .map(|(k, labels)| {
                // calculate the within intra-cluster variation for the reference data sets
                debug!(
                    "Calculating within intra-cluster variation for reference data sets with k={k}"
                );
                let w_kb_log: Vec<_> = reference_datasets
                    .par_iter()
                    .map(|ref_data| {
                        // cluster the reference data into k clusters
                        let ref_labels = self.state.clustering_method.fit(k, ref_data);
                        // calculate the within intra-cluster variation for the reference data
                        let ref_pairwise_distances = calc_pairwise_distances(
                            ref_data.records().view(),
                            k,
                            ref_labels.view(),
                        );
                        calc_within_dispersion(ref_labels.view(), k, ref_pairwise_distances.view())
                            .log2()
                    })
                    .collect();

                // calculate the within intra-cluster variation for the observed data
                let pairwise_distances =
                    calc_pairwise_distances(self.state.embeddings.view(), k, labels.view());
                let w_k = calc_within_dispersion(labels.view(), k, pairwise_distances.view());

                // finally, calculate the gap statistic
                let w_kb_log_sum: f64 = w_kb_log.iter().sum();
                // original formula: l = (1 / B) * sum_b(log(W_kb))
                let l = b.recip() * w_kb_log_sum;
                // original formula: gap_k = (1 / B) * sum_b(log(W_kb)) - log(W_k)
                let gap_k = l - w_k.log2();
                // original formula: sd_k = [(1 / B) * sum_b((log(W_kb) - l)^2)]^0.5
                let standard_deviation = (b.recip()
                    * w_kb_log
                        .iter()
                        .map(|w_kb_log| (w_kb_log - l).powi(2))
                        .sum::<f64>())
                .sqrt();
                // original formula: s_k = sd_k * (1 + 1 / B)^0.5
                // calculate differently to minimize rounding errors
                let s_k = standard_deviation * (1.0 + b.recip()).sqrt();

                (k, gap_k, s_k)
            });

        // // plot the gap_k (whisker with s_k) w.r.t. k
        // #[cfg(feature = "plot_gap")]
        // plot_gap_statistic(results.clone().collect::<Vec<_>>());

        // finally, consume the results iterator until we find the optimal k
        let (mut optimal_k, mut gap_k_minus_one) = (None, None);
        for (k, gap_k, s_k) in results {
            info!("k: {k}, gap_k: {gap_k}, s_k: {s_k}");

            if let Some(gap_k_minus_one) = gap_k_minus_one
                && gap_k_minus_one >= gap_k - s_k
            {
                info!("Optimal k found: {}", k - 1);
                optimal_k = Some(k - 1);
                break;
            }

            gap_k_minus_one = Some(gap_k);
        }

        optimal_k.ok_or(ClusteringError::OptimalKNotFound(self.state.k_max))
    }

    fn get_optimal_k_davies_bouldin(&self) -> Result<usize, ClusteringError> {
        todo!();
    }
}

/// Convert a vector of Analyses into a 2D array
///
/// # Panics
///
/// Will panic if the shape of the data does not match the number of features, should never happen
#[must_use]
#[inline]
pub fn convert_to_array(data: Vec<Analysis>) -> AnalysisArray {
    // Convert vector to Array
    let shape = (data.len(), NUMBER_FEATURES);
    debug_assert_eq!(NUMBER_FEATURES, data[0].inner().len());

    AnalysisArray(
        Array2::from_shape_vec(shape, data.into_iter().flat_map(|a| *a.inner()).collect())
            .expect("Failed to convert to array, shape mismatch"),
    )
}

/// Generate B reference data sets with a random uniform distribution
///
/// (excerpt from reference paper)
/// """
/// We consider two choices for the reference distribution:
///
/// 1. generate each reference feature uniformly over the range of the observed values for that feature.
/// 2. generate the reference features from a uniform distribution over a box aligned with the
///    principle components of the data.
///    In detail, if X is our n by p data matrix, we assume that the columns have mean 0 and compute
///    the singular value decomposition X = UDV^T. We transform via X' = XV and then draw uniform features Z'
///    over the ranges of the columns of X', as in method (1) above.
///    Finally, we back-transform via Z=Z'V^T to give reference data Z.
///
/// Method (1) has the advantage of simplicity. Method (2) takes into account the shape of the
/// data distribution and makes the procedure rotationally invariant, as long as the
/// clustering method itself is invariant
/// """
///
/// For now, we will use method (1) as it is simpler to implement
/// and we know that our data is already normalized and that
/// the ordering of features is important, meaning that we can't
/// rotate the data anyway.
fn generate_reference_datasets(samples: ArrayView2<Feature>, b: usize) -> Vec<FitDataset> {
    (0..b)
        .into_par_iter()
        .map(|_| Dataset::from(generate_ref_single(samples.view())))
        .collect()
}
fn generate_ref_single(samples: ArrayView2<Feature>) -> Array2<Feature> {
    let feature_distributions = samples
        .axis_iter(Axis(1))
        .map(|feature| Array::random(feature.dim(), Uniform::new(feature.min(), feature.max())))
        .collect::<Vec<_>>();
    let feature_dists_views = feature_distributions
        .iter()
        .map(ndarray::ArrayBase::view)
        .collect::<Vec<_>>();
    ndarray::stack(Axis(0), &feature_dists_views)
        .unwrap()
        .t()
        .to_owned()
}

/// Calculate `W_k`, the within intra-cluster variation for the given clustering
///
/// `W_k = \sum_{r=1}^{k} \frac{D_r}{2*n_r}`
fn calc_within_dispersion(
    labels: ArrayView1<usize>,
    k: usize,
    pairwise_distances: ArrayView1<Feature>,
) -> Feature {
    debug_assert_eq!(k, labels.iter().max().unwrap() + 1);

    // we first need to convert our list of labels into a list of the number of samples in each cluster
    let counts = labels.iter().fold(vec![0u32; k], |mut counts, &label| {
        counts[label] += 1;
        counts
    });
    // then, we calculate the within intra-cluster variation
    counts
        .iter()
        .zip(pairwise_distances.iter())
        .map(|(&count, distance)| (2.0 * f64::from(count)).recip() * distance)
        .sum()
}

/// Calculate the `D_r` array, the sum of the pairwise distances in cluster r, for all clusters in the given clustering
///
/// # Arguments
///
/// - `samples`: The samples in the dataset
/// - `k`: The number of clusters
/// - `labels`: The cluster labels for each sample
fn calc_pairwise_distances(
    samples: ArrayView2<Feature>,
    k: usize,
    labels: ArrayView1<usize>,
) -> Array1<Feature> {
    debug_assert_eq!(
        samples.nrows(),
        labels.len(),
        "Samples and labels must have the same length"
    );
    debug_assert_eq!(
        k,
        labels.iter().max().unwrap() + 1,
        "Labels must be in the range 0..k"
    );

    // for each cluster, calculate the sum of the pairwise distances between samples in that cluster
    let mut distances = Array1::zeros(k);
    let mut clusters = vec![Vec::new(); k];
    // build clusters
    for (sample, label) in samples.outer_iter().zip(labels.iter()) {
        clusters[*label].push(sample);
    }
    // calculate pairwise dist. within each cluster
    for (k, cluster) in clusters.iter().enumerate() {
        let mut pairwise_dists = 0.;
        for i in 0..cluster.len() - 1 {
            let a = cluster[i];
            let rest = &cluster[i + 1..];
            for &b in rest {
                pairwise_dists += L2Dist.distance(a, b);
            }
        }
        distances[k] += pairwise_dists + pairwise_dists;
    }
    distances
}

/// Functions available for Initialized state
impl ClusteringHelper<Initialized> {
    /// Cluster the data into k clusters
    ///
    /// # Errors
    ///
    /// Will return an error if the clustering fails
    #[must_use]
    #[inline]
    pub fn cluster(self) -> ClusteringHelper<Finished> {
        let Initialized {
            clustering_method,
            embeddings,
            k,
        } = self.state;

        let embedding_dataset = Dataset::from(embeddings);
        let labels = clustering_method.fit(k, &embedding_dataset);

        ClusteringHelper {
            state: Finished { labels, k },
        }
    }
}

/// Functions available for Finished state
impl ClusteringHelper<Finished> {
    /// use the labels to reorganize the provided samples into clusters
    #[must_use]
    #[inline]
    pub fn extract_analysis_clusters<T: Clone>(&self, samples: Vec<T>) -> Vec<Vec<T>> {
        let mut clusters = vec![Vec::new(); self.state.k];

        for (sample, &label) in samples.into_iter().zip(self.state.labels.iter()) {
            clusters[label].push(sample);
        }

        clusters
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ndarray::{arr1, arr2, s};
    use ndarray_rand::rand_distr::StandardNormal;
    use pretty_assertions::assert_eq;
    use rstest::rstest;

    #[test]
    fn test_generate_reference_data_set() {
        let data = arr2(&[[10.0, -10.0], [20.0, -20.0], [30.0, -30.0]]);

        let ref_data = generate_ref_single(data.view());

        // First column all vals between 10.0 and 30.0
        assert!(
            ref_data
                .slice(s![.., 0])
                .iter()
                .all(|v| *v >= 10.0 && *v <= 30.0)
        );

        // Second column all vals between -10.0 and -30.0
        assert!(
            ref_data
                .slice(s![.., 1])
                .iter()
                .all(|v| *v <= -10.0 && *v >= -30.0)
        );

        // check that the shape is correct
        assert_eq!(ref_data.shape(), data.shape());

        // check that the data is not the same as the original data
        assert_ne!(ref_data, data);
    }

    #[test]
    fn test_pairwise_distances() {
        let samples = arr2(&[[1.0, 1.0], [1.0, 1.0], [2.0, 2.0], [2.0, 2.0]]);
        let labels = arr1(&[0, 0, 1, 1]);

        let pairwise_distances = calc_pairwise_distances(samples.view(), 2, labels.view());

        assert!(
            f64::EPSILON > (pairwise_distances[0] - 0.0).abs(),
            "{} != 0.0",
            pairwise_distances[0]
        );
        assert!(
            f64::EPSILON > (pairwise_distances[1] - 0.0).abs(),
            "{} != 0.0",
            pairwise_distances[1]
        );

        let samples = arr2(&[[1.0, 2.0], [1.0, 1.0], [2.0, 2.0], [2.0, 3.0]]);

        let pairwise_distances = calc_pairwise_distances(samples.view(), 2, labels.view());

        assert!(
            f64::EPSILON > (pairwise_distances[0] - 2.0).abs(),
            "{} != 2.0",
            pairwise_distances[0]
        );
        assert!(
            f64::EPSILON > (pairwise_distances[1] - 2.0).abs(),
            "{} != 2.0",
            pairwise_distances[1]
        );
    }

    #[test]
    fn test_convert_to_vec() {
        let data = vec![
            Analysis::new([1.0; NUMBER_FEATURES]),
            Analysis::new([2.0; NUMBER_FEATURES]),
            Analysis::new([3.0; NUMBER_FEATURES]),
        ];

        let array = convert_to_array(data);

        assert_eq!(array.0.shape(), &[3, NUMBER_FEATURES]);
        assert!(
            f64::EPSILON > (array.0[[0, 0]] - 1.0).abs(),
            "{} != 1.0",
            array.0[[0, 0]]
        );
        assert!(
            f64::EPSILON > (array.0[[1, 0]] - 2.0).abs(),
            "{} != 2.0",
            array.0[[1, 0]]
        );
        assert!(
            f64::EPSILON > (array.0[[2, 0]] - 3.0).abs(),
            "{} != 3.0",
            array.0[[2, 0]]
        );

        // check that axis iteration works how we expect
        // axis 0
        let mut iter = array.0.axis_iter(Axis(0));
        assert_eq!(iter.next().unwrap().to_vec(), vec![1.0; NUMBER_FEATURES]);
        assert_eq!(iter.next().unwrap().to_vec(), vec![2.0; NUMBER_FEATURES]);
        assert_eq!(iter.next().unwrap().to_vec(), vec![3.0; NUMBER_FEATURES]);
        // axis 1
        for column in array.0.axis_iter(Axis(1)) {
            assert_eq!(column.to_vec(), vec![1.0, 2.0, 3.0]);
        }
    }

    #[test]
    fn test_calc_within_dispersion() {
        let labels = arr1(&[0, 1, 0, 1]);
        let pairwise_distances = arr1(&[1.0, 2.0]);
        let result = calc_within_dispersion(labels.view(), 2, pairwise_distances.view());

        // `W_k = \sum_{r=1}^{k} \frac{D_r}{2*n_r}` = 1/4 * 1.0 + 1/4 * 2.0 = 0.25 + 0.5 = 0.75
        assert!(f64::EPSILON > (result - 0.75).abs(), "{result} != 0.75");
    }

    #[rstest]
    #[case::project_none(ProjectionMethod::None, NUMBER_FEATURES)]
    #[case::project_tsne(ProjectionMethod::TSne, EMBEDDING_SIZE)]
    #[case::project_pca(ProjectionMethod::Pca, EMBEDDING_SIZE)]
    fn test_project(
        #[case] projection_method: ProjectionMethod,
        #[case] expected_embedding_size: usize,
    ) {
        // generate 100 random samples, we use a normal distribution because with a uniform distribution
        // the data has no real "principle components" and PCA will not work as expected since almost all the eigenvalues
        // with fall below the cutoff
        let mut samples = Array2::random((100, NUMBER_FEATURES), StandardNormal);
        normalize_embeddings_inplace::<NUMBER_FEATURES>(&mut samples);
        let samples = AnalysisArray(samples);

        let result = projection_method.project(samples).unwrap();

        // ensure embeddings are the correct shape
        assert_eq!(result.shape(), &[100, expected_embedding_size]);

        // ensure the data is normalized
        for i in 0..expected_embedding_size {
            let min = result.column(i).min();
            let max = result.column(i).max();
            assert!(
                f64::EPSILON > (min + 1.0).abs(),
                "Min value of column {i} is not -1.0: {min}",
            );
            assert!(
                f64::EPSILON > (max - 1.0).abs(),
                "Max value of column {i} is not 1.0: {max}",
            );
        }
    }
}

// #[cfg(feature = "plot_gap")]
// fn plot_gap_statistic(data: Vec<(usize, f64, f64)>) {
//     use plotters::prelude::*;

//     // Assuming data is a Vec<(usize, f64, f64)> of (k, gap_k, s_k)
//     let root_area = BitMapBackend::new("gap_statistic_plot.png", (640, 480)).into_drawing_area();
//     root_area.fill(&WHITE).unwrap();

//     let max_gap_k = data
//         .iter()
//         .map(|(_, gap_k, _)| *gap_k)
//         .fold(f64::MIN, f64::max);
//     let min_gap_k = data
//         .iter()
//         .map(|(_, gap_k, _)| *gap_k)
//         .fold(f64::MAX, f64::min);
//     let max_k = data.iter().map(|(k, _, _)| *k).max().unwrap_or(0);

//     let mut chart = ChartBuilder::on(&root_area)
//         .caption("Gap Statistic Plot", ("sans-serif", 30))
//         .margin(5)
//         .x_label_area_size(30)
//         .y_label_area_size(30)
//         .build_cartesian_2d(0..max_k, min_gap_k..max_gap_k)
//         .unwrap();

//     chart.configure_mesh().draw().unwrap();

//     for (k, gap_k, s_k) in data {
//         chart
//             .draw_series(PointSeries::of_element(
//                 vec![(k, gap_k)],
//                 5,
//                 &RED,
//                 &|coord, size, style| {
//                     EmptyElement::at(coord) + Circle::new((0, 0), size, style.filled())
//                 },
//             ))
//             .unwrap();

//         // Drawing error bars
//         chart
//             .draw_series(LineSeries::new(
//                 vec![(k, gap_k - s_k), (k, gap_k + s_k)],
//                 &BLACK,
//             ))
//             .unwrap();
//     }

//     root_area.present().unwrap();
// }
