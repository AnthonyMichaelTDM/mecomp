use anyhow::Result;
use linfa::prelude::*;
use linfa_clustering::{GaussianMixtureModel, KMeans};
use linfa_tsne::TSneParams;
use mecomp_core::get_data_dir;
use mecomp_storage::db::{
    schemas::{analysis::Analysis, collection::Collection},
    set_database_path,
};
use ndarray::Array2;
use plotters::prelude::*;

// mod pca;
// use pca::Pca;

#[tokio::main]
async fn main() -> Result<()> {
    // parse number of clusters from command line arguments
    let k = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "3".to_string())
        .parse::<usize>()?;
    // let min_points = std::env::args()
    //     .nth(2)
    //     .unwrap_or("5".to_string())
    //     .parse::<usize>()?;
    // let tolerance = std::env::args()
    //     .nth(3)
    //     .unwrap_or("0.01".to_string())
    //     .parse::<f64>()?;

    let db_dir = get_data_dir()?.join("db");
    set_database_path(db_dir)?;

    let analysis = collect_analyses().await?;

    // project the analyses into 2D space using t-SNE
    let embedding = project_tsne(analysis)?;
    let embedding_dataset = Dataset::from(embedding.clone());

    scatter_plot(&embedding, "tsne")?;

    // cluster the analyses using k-means
    println!("clustering analyses using k-means");
    let kmeans = KMeans::params(k).fit(&embedding_dataset)?;
    let kmeans_labels = kmeans.predict(&embedding_dataset);
    scatter_plot_with_clusters(&embedding, kmeans_labels.as_slice().unwrap(), "tsne-kmeans")?;

    // cluster the analyses using GMM
    println!("clustering analyses using GMM");
    let gmm = GaussianMixtureModel::params(k).fit(&embedding_dataset)?;
    let gmm_labels = gmm.predict(&embedding_dataset);
    scatter_plot_with_clusters(&embedding, gmm_labels.as_slice().unwrap(), "tsne-gmm")?;

    // print the collections currently in the database
    display_collections().await?;

    // // cluster the analyses with DBSCAN
    // println!("clustering analyses using DBSCAN");
    // let dbscan_labels = Dbscan::params(min_points)
    //     .tolerance(tolerance)
    //     .transform(&embedding)?
    //     .into_iter()
    //     .map(|l| l.unwrap_or(k + 1))
    //     .collect::<Vec<_>>();
    // scatter_plot_with_clusters(&embedding, &dbscan_labels, "tsne-dbscan")?;

    Ok(())
}

async fn collect_analyses() -> Result<Array2<f64>> {
    let connection = mecomp_storage::db::init_database().await?;

    println!("connected to database");

    // collect all analysis from the database
    let analysis = Analysis::read_all(&connection)
        .await?
        .into_iter()
        .flat_map(|a| a.features)
        .collect::<Vec<_>>();

    drop(connection);

    println!("collected {} analyses", analysis.len() / 20);

    let analysis: ndarray::ArrayBase<ndarray::OwnedRepr<f64>, ndarray::Dim<[usize; 2]>> =
        ndarray::Array2::from_shape_vec((analysis.len() / 20, 20), analysis)?;

    println!("analysis shape: {:?}", analysis.shape());

    Ok(analysis)
}

// #[allow(dead_code)]
// fn project_pca(data: Array2<f64>) -> Result<Array2<f64>> {
//     println!("projecting data into 2D space (PCA)");

//     let data = Dataset::from(data);

//     let pca = Pca::params(2).fit(&data)?;
//     let embedding = pca.transform(data);
//     let embedding = embedding.records;

//     println!("embedding shape: {:?}", embedding.shape());

//     Ok(embedding)
// }

fn project_tsne(data: Array2<f64>) -> Result<Array2<f64>> {
    println!("projecting data into 2D space (t-SNE)");

    // let data = Dataset::from(data);
    #[allow(clippy::cast_precision_loss)]
    let embedding = TSneParams::embedding_size(2)
        .perplexity(61.)
        .approx_threshold(0.5)
        .transform(data)?;

    Ok(embedding)
}

fn scatter_plot(data: &Array2<f64>, name: &str) -> Result<()> {
    println!("generating scatter plot");
    assert_eq!(data.ncols(), 2, "data must have 2 columns");

    let plot_name = format!("scatter-{name}.svg");
    let plot = SVGBackend::new(&plot_name, (1024, 1024)).into_drawing_area();

    plot.fill(&WHITE)?;

    let x_range = data
        .column(0)
        .iter()
        .fold(f64::INFINITY..f64::NEG_INFINITY, |acc, &x| {
            acc.start.min(x)..acc.end.max(x)
        });

    let y_range = data
        .column(1)
        .iter()
        .fold(f64::INFINITY..f64::NEG_INFINITY, |acc, &y| {
            acc.start.min(y)..acc.end.max(y)
        });

    let mut chart = ChartBuilder::on(&plot)
        .x_label_area_size(40)
        .y_label_area_size(40)
        .margin(5)
        .caption(name, ("sans-serif", 50))
        .build_cartesian_2d(x_range, y_range)?;

    chart.configure_mesh().draw()?;

    chart.draw_series(
        data.outer_iter()
            .map(|point| Circle::new((point[0], point[1]), 2, BLUE.filled())),
    )?;

    plot.present()?;

    Ok(())
}

fn scatter_plot_with_clusters(data: &Array2<f64>, labels: &[usize], name: &str) -> Result<()> {
    println!("generating scatter plot with clusters");
    assert_eq!(data.ncols(), 2, "data must have 2 columns");

    let plot_name = format!("scatter-{name}.svg");
    let plot = SVGBackend::new(&plot_name, (1024, 1024)).into_drawing_area();

    plot.fill(&WHITE)?;

    let x_range = data
        .column(0)
        .iter()
        .fold(f64::INFINITY..f64::NEG_INFINITY, |acc, &x| {
            acc.start.min(x)..acc.end.max(x)
        });

    let y_range = data
        .column(1)
        .iter()
        .fold(f64::INFINITY..f64::NEG_INFINITY, |acc, &y| {
            acc.start.min(y)..acc.end.max(y)
        });

    let mut chart = ChartBuilder::on(&plot)
        .x_label_area_size(40)
        .y_label_area_size(40)
        .margin(5)
        .caption(name, ("sans-serif", 50))
        .build_cartesian_2d(x_range, y_range)?;

    chart.configure_mesh().draw()?;

    let colors = [
        BLUE,
        RED,
        GREEN,
        CYAN,
        MAGENTA,
        BLACK,
        RGBColor(255, 128, 0),
        RGBColor(0, 255, 128),
        RGBColor(128, 0, 255),
        RGBColor(255, 0, 128),
        RGBColor(0, 128, 255),
        RGBColor(128, 255, 0),
    ];

    chart.draw_series(data.outer_iter().zip(labels.iter()).map(|(point, &label)| {
        Circle::new(
            (point[0], point[1]),
            2,
            colors[label % colors.len()].filled(),
        )
    }))?;

    plot.present()?;

    Ok(())
}

async fn display_collections() -> Result<()> {
    let connection = mecomp_storage::db::init_database().await?;
    let mut analysis_collection_pairs = Vec::with_capacity(1224);
    for (i, collection) in Collection::read_all(&connection)
        .await?
        .into_iter()
        .enumerate()
    {
        let songs = Collection::read_songs(&connection, collection.id.clone()).await?;
        let analyses =
            Analysis::read_for_songs(&connection, songs.into_iter().map(|s| s.id).collect())
                .await?;
        analysis_collection_pairs.extend(analyses.into_iter().filter_map(|a| a.map(|a| (a, i))));
    }
    let collection_embeddings = project_tsne(ndarray::Array2::from_shape_vec(
        (analysis_collection_pairs.len(), 20),
        analysis_collection_pairs
            .iter()
            .flat_map(|(a, _)| a.features)
            .collect(),
    )?)?;
    let collection_labels = analysis_collection_pairs
        .iter()
        .map(|(_, i)| *i)
        .collect::<Vec<_>>();
    scatter_plot_with_clusters(&collection_embeddings, &collection_labels, "collections")?;

    Ok(())
}
