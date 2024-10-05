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
use linfa_clustering::KMeans;
use linfa_nn::distance::{Distance, L2Dist};
use linfa_tsne::TSneParams;
use log::{debug, info};
use ndarray::{Array, Array1, Array2, ArrayView1, ArrayView2, Axis};
use ndarray_rand::RandomExt;
use rand::distributions::Uniform;
use rayon::iter::{IntoParallelRefIterator, ParallelIterator};
use statrs::statistics::Statistics;

use crate::{errors::ClusteringError, Analysis, Feature, NUMBER_FEATURES};

pub struct AnalysisArray(pub(crate) Array2<Feature>);

impl From<Vec<Analysis>> for AnalysisArray {
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
    fn from(data: Vec<[Feature; NUMBER_FEATURES]>) -> Self {
        let shape = (data.len(), NUMBER_FEATURES);
        debug_assert_eq!(shape, (data.len(), data[0].len()));

        Self(
            Array2::from_shape_vec(shape, data.into_iter().flatten().collect())
                .expect("Failed to convert to array, shape mismatch"),
        )
    }
}

pub enum KOptimal {
    GapStatistic {
        /// The number of reference datasets to generate
        b: usize,
    },
    DaviesBouldin,
}

// log the number of features
const EMBEDDING_SIZE: usize = 2;
// {
//     let log2 = usize::ilog2(NUMBER_FEATURES) as usize;
//     if log2 < 2 {
//         2
//     } else {
//         log2
//     }
// };

// #[cfg(debug_assertions)]
// const MAX_ITERATIONS: u64 = 50;
// #[cfg(not(debug_assertions))]
// const MAX_ITERATIONS: u64 = 100;

pub struct KMeansHelper<S>
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
}
pub struct Initialized {
    /// The embeddings of our input, as a Nx`EMBEDDING_SIZE` array
    embeddings: Array2<Feature>,
    pub k: usize,
}
pub struct Finished {
    /// The labelings of the samples, as a Nx1 array.
    /// Each element is the cluster that the corresponding sample belongs to.
    labels: Array1<usize>,
    pub k: usize,
}

/// Functions available for all states
impl KMeansHelper<EntryPoint> {
    /// Create a new `KMeansHelper` object
    ///
    /// # Errors
    ///
    /// Will return an error if there was an error projecting the data into a lower-dimensional space
    pub fn new(
        samples: AnalysisArray,
        k_max: usize,
        optimizer: KOptimal,
    ) -> Result<KMeansHelper<NotInitialized>, ClusteringError> {
        // first use the t-SNE algorithm to project the data into a lower-dimensional space
        debug!("Generating embeddings (size: {EMBEDDING_SIZE}) using t-SNE",);

        #[allow(clippy::cast_precision_loss)]
        let mut embeddings = TSneParams::embedding_size(EMBEDDING_SIZE)
            .perplexity(f64::clamp(samples.0.nrows() as f64 / 100., 5.0, 50.0))
            .approx_threshold(0.5)
            .max_iter(1000)
            .transform(samples.0)?;

        debug!("Embeddings shape: {:?}", embeddings.shape());

        // normalize the embeddings so each dimension is between -1 and 1
        debug!("Normalizing embeddings");
        for i in 0..EMBEDDING_SIZE {
            let min = embeddings.column(i).min();
            let max = embeddings.column(i).max();
            let range = max - min;
            embeddings
                .column_mut(i)
                .mapv_inplace(|v| ((v - min) / range).mul_add(2., -1.));
        }

        Ok(KMeansHelper {
            state: NotInitialized {
                embeddings,
                k_max,
                optimizer,
            },
        })
    }
}

/// Functions available for `NotInitialized` state
impl KMeansHelper<NotInitialized> {
    /// Initialize the `KMeansHelper` object
    ///
    /// # Errors
    ///
    /// Will return an error if there was an error calculating the optimal number of clusters
    pub fn initialize(self) -> Result<KMeansHelper<Initialized>, ClusteringError> {
        let k = self.get_optimal_k()?;
        Ok(KMeansHelper {
            state: Initialized {
                embeddings: self.state.embeddings,
                k,
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
    ///    `Gap(k)=(1/B) \sum_{b=1}^{B} \log(W^*_{kb})−\log(W_k)`.
    ///    Compute also the standard deviation of the statistics.
    /// 4. Choose the number of clusters as the smallest value of k such that the gap statistic is within one standard deviation of the gap at k+1:
    ///    `Gap(k)≥Gap(k + 1)−s_{k + 1}`.
    fn get_optimal_k_gap_statistic(&self, b: usize) -> Result<usize, ClusteringError> {
        // our reference data sets
        let reference_data_sets = generate_reference_data_set(self.state.embeddings.view(), b);

        let mut results = (1..=self.state.k_max)
            // for each k, cluster the data into k clusters
            .map(|k| {
                let embedding_dataset = Dataset::from(self.state.embeddings.clone());
                debug!("Fitting k-means to embeddings with k={k}");
                let model = KMeans::params(k)
                    // .max_n_iterations(MAX_ITERATIONS)
                    .fit(&embedding_dataset)
                    .unwrap();
                let labels = model.predict(&self.state.embeddings);
                (k, labels)
            })
            // for each k, calculate the gap statistic, and the standard deviation of the statistics
            .map(|(k, labels)| {
                // first, we calculate the within intra-cluster variation for the observed data
                let pairwise_distances =
                    calc_pairwise_distances(self.state.embeddings.view(), k, labels.view());
                let w_k = calc_within_dispersion(labels.view(), k, pairwise_distances.view());

                // then, we calculate the within intra-cluster variation for the reference data sets
                debug!(
                    "Calculating within intra-cluster variation for reference data sets with k={k}"
                );
                let w_kb = reference_data_sets.par_iter().map(|ref_data| {
                    // cluster the reference data into k clusters
                    let binding = Dataset::from(ref_data.clone());
                    let ref_labels = KMeans::params(k)
                        // .max_n_iterations(MAX_ITERATIONS)
                        .fit(&binding)
                        .unwrap()
                        .predict(ref_data);
                    // calculate the within intra-cluster variation for the reference data
                    let ref_pairwise_distances =
                        calc_pairwise_distances(ref_data.view(), k, ref_labels.view());
                    calc_within_dispersion(ref_labels.view(), k, ref_pairwise_distances.view())
                });

                // finally, we calculate the gap statistic
                #[allow(clippy::cast_precision_loss)]
                let gap_k = (1.0 / b as f64)
                    .mul_add(w_kb.clone().map(f64::log2).sum::<f64>().log2(), -w_k.log2());

                #[allow(clippy::cast_precision_loss)]
                let l = (1.0 / b as f64) * w_kb.clone().map(f64::log2).sum::<f64>();
                #[allow(clippy::cast_precision_loss)]
                let standard_deviation = ((1.0 / b as f64)
                    * w_kb.map(|w_kb| (w_kb.log2() - l).powi(2)).sum::<f64>())
                .sqrt();
                #[allow(clippy::cast_precision_loss)]
                let s_k = standard_deviation * (1.0 + 1.0 / b as f64).sqrt();

                (k, gap_k, s_k)
            });
        // // now, we have to bring the iterator back to a single thread
        // .collect::<Vec<_>>()
        // .into_iter();

        // // plot the gap_k (whisker with s_k) w.r.t. k
        // #[cfg(feature = "plot_gap")]
        // plot_gap_statistic(results.clone().collect::<Vec<_>>());

        // finally, we go over the iterator to find the optimal k
        let (mut optimal_k, mut gap_k_minus_one) =
            (None, results.next().map(|(_, gap_k, _)| gap_k));

        for (k, gap_k, s_k) in results {
            info!("k: {k}, gap_k: {gap_k}, s_k: {s_k}");

            if let Some(gap_k_minus_one) = gap_k_minus_one {
                if gap_k_minus_one >= gap_k - s_k {
                    info!("Optimal k found: {}", k - 1);
                    optimal_k = Some(k - 1);
                    break;
                }
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
pub fn convert_to_array(data: Vec<Analysis>) -> AnalysisArray {
    // Convert vector to Array
    let shape = (data.len(), NUMBER_FEATURES);
    debug_assert_eq!(shape, (data.len(), data[0].inner().len()));

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
fn generate_reference_data_set(samples: ArrayView2<Feature>, b: usize) -> Vec<Array2<f64>> {
    let mut reference_data_sets = Vec::with_capacity(b);
    for _ in 0..b {
        reference_data_sets.push(generate_ref_single(samples));
    }

    reference_data_sets
}
fn generate_ref_single(samples: ArrayView2<Feature>) -> Array2<f64> {
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
    let counts = labels.iter().fold(vec![0; k], |mut counts, &label| {
        counts[label] += 1;
        counts
    });
    // then, we calculate the within intra-cluster variation
    counts
        .iter()
        .zip(pairwise_distances.iter())
        .map(|(&count, distance)| distance / (2.0 * f64::from(count)))
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

    // for each cluster, calculate the sum of the pairwise distances betweeen samples in that cluster
    (0..k)
        .map(|k| {
            (
                k,
                samples
                    .outer_iter()
                    .zip(labels.iter())
                    .filter_map(|(s, &l)| (l == k).then_some(s))
                    .collect::<Vec<_>>(),
            )
        })
        .fold(Array1::zeros(k), |mut distances, (label, cluster)| {
            distances[label] += cluster
                .iter()
                .enumerate()
                .map(|(i, &a)| {
                    cluster
                        .iter()
                        .skip(i + 1)
                        .map(|&b| L2Dist.distance(a, b))
                        .sum::<Feature>()
                })
                .sum::<Feature>();
            distances
        })
}

/// Functions available for Initialized state
impl KMeansHelper<Initialized> {
    /// Cluster the data into k clusters
    ///
    /// # Errors
    ///
    /// Will return an error if the clustering fails
    pub fn cluster(self, max_iterations: u64) -> Result<KMeansHelper<Finished>, ClusteringError> {
        let model = KMeans::params(self.state.k)
            .max_n_iterations(max_iterations)
            .fit(&Dataset::from(self.state.embeddings.clone()))?;
        let labels = model.predict(&self.state.embeddings);

        Ok(KMeansHelper {
            state: Finished {
                labels,
                k: self.state.k,
            },
        })
    }
}

/// Functions available for Finished state
impl KMeansHelper<Finished> {
    /// use the labels to reorganize the provided samples into clusters
    #[must_use]
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
    use pretty_assertions::assert_eq;

    #[test]
    fn test_generate_reference_data_set() {
        let data = arr2(&[[10.0, -10.0], [20.0, -20.0], [30.0, -30.0]]);

        let ref_data = generate_ref_single(data.view());

        // First column all vals between 10.0 and 30.0
        assert!(ref_data
            .slice(s![.., 0])
            .iter()
            .all(|v| *v >= 10.0 && *v <= 30.0));

        // Second column all vals between -10.0 and -30.0
        assert!(ref_data
            .slice(s![.., 1])
            .iter()
            .all(|v| *v <= -10.0 && *v >= -30.0));

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

        assert_eq!(pairwise_distances[0], 0.0);
        assert_eq!(pairwise_distances[1], 0.0);

        let samples = arr2(&[[1.0, 2.0], [1.0, 1.0], [2.0, 2.0], [2.0, 3.0]]);

        let pairwise_distances = calc_pairwise_distances(samples.view(), 2, labels.view());

        assert_eq!(pairwise_distances[0], 1.0);
        assert_eq!(pairwise_distances[1], 1.0);
    }

    #[test]
    fn test_convert_to_vec() {
        let data = vec![
            Analysis::new([1.0; NUMBER_FEATURES]),
            Analysis::new([2.0; NUMBER_FEATURES]),
            Analysis::new([3.0; NUMBER_FEATURES]),
        ];

        let array = convert_to_array(data.clone());

        assert_eq!(array.0.shape(), &[3, NUMBER_FEATURES]);
        assert_eq!(array.0[[0, 0]], 1.0);
        assert_eq!(array.0[[1, 0]], 2.0);
        assert_eq!(array.0[[2, 0]], 3.0);

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
