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

use clustering::{kmeans, Clustering};
use ndarray::{Array, Array1, Array2, ArrayView1, ArrayView2, Axis};
use ndarray_rand::RandomExt;
use rand::distributions::Uniform;
use rayon::iter::{IntoParallelIterator, IntoParallelRefIterator, ParallelIterator};
use statrs::statistics::Statistics;

use crate::{errors::ClusteringError, Analysis, Feature, NUMBER_FEATURES};

// re-export of the Elem trait from the clustering crate
pub use clustering::Elem;

pub trait Sample: Elem + Sync {
    fn inner(&self) -> &[f64; NUMBER_FEATURES];
}

impl Elem for Analysis {
    fn dimensions(&self) -> usize {
        NUMBER_FEATURES
    }

    fn at(&self, i: usize) -> f64 {
        self.internal_analysis[i]
    }
}

impl Sample for Analysis {
    fn inner(&self) -> &[f64; NUMBER_FEATURES] {
        &self.internal_analysis
    }
}

/// use to cluster on ndarray arrays without having to convert them fully to vectors every time
struct AnalysisArray1<'a>(ArrayView1<'a, f64>);
impl<'a> From<ArrayView1<'a, f64>> for AnalysisArray1<'a> {
    fn from(array: ArrayView1<'a, f64>) -> Self {
        AnalysisArray1(array)
    }
}

impl<'a> Elem for AnalysisArray1<'a> {
    fn dimensions(&self) -> usize {
        self.0.len()
    }

    fn at(&self, i: usize) -> f64 {
        self.0[i]
    }
}

impl<'a> Sample for AnalysisArray1<'a> {
    fn inner(&self) -> &[f64; NUMBER_FEATURES] {
        self.0
            .as_slice()
            .unwrap()
            .try_into()
            .expect("Failed to convert to array")
    }
}

pub enum KOptimal {
    GapStatistic {
        /// The number of reference datasets to generate
        b: usize,
    },
    DaviesBouldin,
}

const MAX_ITERATIONS: usize = 30;

pub struct KMeansHelper<S>
where
    S: Sized,
{
    state: S,
}

pub struct EntryPoint;
pub struct NotInitialized<T: Sample> {
    samples: Vec<T>,
    pub k_max: usize,
    pub optimizer: KOptimal,
}
pub struct Initialized<T: Sample> {
    samples: Vec<T>,
    pub k: usize,
}

/// Functions available for all states
impl KMeansHelper<EntryPoint> {
    #[must_use]
    pub fn new<T: Sample>(
        raw_samples: Vec<T>,
        k_max: usize,
        optimizer: KOptimal,
    ) -> KMeansHelper<NotInitialized<T>> {
        // finally, we can create the kmeans object
        KMeansHelper {
            state: NotInitialized {
                samples: raw_samples,
                k_max,
                optimizer,
            },
        }
    }
}

/// Functions available for `NotInitialized` state
impl<T: Sample> KMeansHelper<NotInitialized<T>> {
    /// Initialize the `KMeansHelper` object
    ///
    /// # Errors
    ///
    /// Will return an error if there was an error calculating the optimal number of clusters
    pub fn initialize(self) -> Result<KMeansHelper<Initialized<T>>, ClusteringError> {
        let k = self.get_optimal_k()?;
        Ok(KMeansHelper {
            state: Initialized {
                samples: self.state.samples,
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
        let reference_data_sets =
            generate_reference_data_set(&convert_to_array(&self.state.samples).view(), b);

        let (optimal_k, _) = (1..=self.state.k_max)
            .into_par_iter()
            // for each k, cluster the data into k clusters
            .map(|k| {
                (
                    k,
                    extract_clusters(kmeans(k, &self.state.samples, MAX_ITERATIONS)),
                )
            })
            // for each k, calculate the gap statistic, and the standard deviation of the statistics
            .map(|(k, clusters)| {
                // first, we calculate the within intra-cluster variation for the observed data
                let pairwise_distances = calc_pairwise_distances(&clusters);
                let w_k = calc_within_dispersion(&clusters, &pairwise_distances);

                // then, we calculate the within intra-cluster variation for the reference data sets
                let w_kb = reference_data_sets.par_iter().map(|ref_data| {
                    // cluster the reference data into k clusters
                    let binding = ref_data
                        .axis_iter(Axis(0))
                        .map(AnalysisArray1::from)
                        .collect::<Vec<_>>();
                    let ref_clusters = extract_clusters(kmeans(k, &binding, MAX_ITERATIONS));
                    // calculate the within intra-cluster variation for the reference data
                    let ref_pairwise_distances = calc_pairwise_distances(&ref_clusters);
                    calc_within_dispersion(&ref_clusters, &ref_pairwise_distances)
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
            })
            // now, we fold over the iterator to find the optimal k
            // but first, we have to bring the iterator back to a single thread
            .collect::<Vec<_>>()
            .into_iter()
            .fold(
                (None, None),
                |(mut optimal_k, gap_k), (k, gap_k_plus_one, s_k_plus_one)| {
                    if let Some(gap_k) = gap_k {
                        if gap_k >= gap_k_plus_one - s_k_plus_one {
                            optimal_k = Some(k);
                        }
                    }
                    (optimal_k, Some(gap_k_plus_one))
                },
            );

        optimal_k.ok_or(ClusteringError::OptimalKNotFound(self.state.k_max))
    }

    fn get_optimal_k_davies_bouldin(&self) -> Result<usize, ClusteringError> {
        todo!();
    }
}

/// TODO: eventually we will want to not need to do this
fn convert_to_array<T: Sample>(data: &[T]) -> Array2<f64> {
    // Convert vector to Array
    let shape = (data.len(), NUMBER_FEATURES);
    //let mut array = Array2::zeros(shape);
    let data = Array1::from_iter(data.iter().flat_map(|v| *v.inner()))
        .into_shape(shape)
        .expect("Failed to reshape!");
    data
}

/// extract the clusters from the given clustering
#[allow(clippy::needless_pass_by_value)]
#[must_use]
pub fn extract_clusters<T: Elem + Sync>(clustering: Clustering<'_, T>) -> Vec<Vec<&T>> {
    let mut clusters = vec![Vec::new(); clustering.centroids.len()];

    // NOTE: if clusters.membership[i] = y, then clusters.elements[i] belongs to cluster y.
    for (i, elem) in clustering.elements.iter().enumerate() {
        clusters[clustering.membership[i]].push(elem);
    }

    clusters
}

/// Generate B reference data sets with a random uniform distribution
///
/// (exerpt from reference paper)
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
/// Method (1) has the advantage of simplicity. Method (b) takes into accound the shape of the
/// data distribution and makes the procedure rotationally invariant, as long as the
/// clustering method itself is invariant
/// """
///
/// For now, we will use method (1) as it is simpler to implement
/// and we know that our data is already normalized and that
/// the ordering of features is important, meaning that we can't
/// rotate the data anyway.
fn generate_reference_data_set(samples: &ArrayView2<Feature>, b: usize) -> Vec<Array2<f64>> {
    let mut reference_data_sets = Vec::with_capacity(b);
    for _ in 0..b {
        reference_data_sets.push(generate_ref_single(samples));
    }

    reference_data_sets
}
fn generate_ref_single(samples: &ArrayView2<Feature>) -> Array2<f64> {
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
fn calc_within_dispersion<T: Elem + Sync>(clusters: &[Vec<&T>], pairwise_distances: &[f64]) -> f64 {
    #[allow(clippy::cast_precision_loss)]
    clusters
        .iter()
        .zip(pairwise_distances.iter())
        .map(|(cluster, distance)| distance / (2.0 * cluster.len() as f64))
        .sum()
}

/// Calculate the `D_r` array, the sum of the pairwise distances in cluster r, for all clusters in the given clustering
fn calc_pairwise_distances<T: Elem + Sync>(clusters: &[Vec<&T>]) -> Vec<f64> {
    let mut distances = vec![0.0; clusters.len()];

    // for each cluster
    for (i, cluster) in clusters.iter().enumerate() {
        // for each element in the cluster
        for (a, b) in cluster.iter().enumerate() {
            for c in cluster.iter().skip(a + 1) {
                distances[i] += distance(*b, *c);
            }
        }
    }

    distances
}

/// Calculate the euclidean distance between two elements
fn distance<T: Elem + Sync>(a: &T, b: &T) -> f64 {
    let mut sum = 0.0;
    for i in 0..a.dimensions() {
        let diff = a.at(i) - b.at(i);
        sum += diff * diff;
    }
    sum.sqrt()
}

/// Functions available for Initialized state
impl<T: Sample> KMeansHelper<Initialized<T>> {
    #[must_use]
    pub fn cluster(&self, max_iterations: usize) -> Clustering<'_, T> {
        kmeans(self.state.k, &self.state.samples, max_iterations)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use ndarray::{arr2, s};
    use pretty_assertions::assert_eq;

    #[test]
    fn test_generate_reference_data_set() {
        let data = arr2(&[[10.0, -10.0], [20.0, -20.0], [30.0, -30.0]]);

        let ref_data = generate_ref_single(&data.view());

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
    fn test_convert_to_vec() {
        let data = vec![
            Analysis::new([1.0; NUMBER_FEATURES]),
            Analysis::new([2.0; NUMBER_FEATURES]),
            Analysis::new([3.0; NUMBER_FEATURES]),
        ];

        let array = convert_to_array(&data);

        assert_eq!(array.shape(), &[3, NUMBER_FEATURES]);
        assert_eq!(array[[0, 0]], 1.0);
        assert_eq!(array[[1, 0]], 2.0);
        assert_eq!(array[[2, 0]], 3.0);

        // check that axis iteration works how we expect
        // axis 0
        let mut iter = array.axis_iter(Axis(0));
        assert_eq!(iter.next().unwrap().to_vec(), vec![1.0; NUMBER_FEATURES]);
        assert_eq!(iter.next().unwrap().to_vec(), vec![2.0; NUMBER_FEATURES]);
        assert_eq!(iter.next().unwrap().to_vec(), vec![3.0; NUMBER_FEATURES]);
        // axis 1
        for column in array.axis_iter(Axis(1)) {
            assert_eq!(column.to_vec(), vec![1.0, 2.0, 3.0]);
        }
    }
}
