#[cfg(feature = "enterprise")]
use fastembed::{EmbeddingModel, InitOptions, TextEmbedding};
#[cfg(feature = "enterprise")]
use std::sync::Arc;
#[cfg(feature = "enterprise")]
use tokio::sync::Mutex;

/// Version string used to detect when re-indexing is needed.
pub const EMBEDDING_MODEL_VERSION: &str = "bge-small-en-v1.5-v1";

/// Batch size for embedding inference. fastembed defaults to 256, which with
/// BGE-small (512 token seq) can spike past 4 GiB in ONNX attention
/// activations. 8 keeps per-call peak memory well under a gig at the cost of
/// more iterations — acceptable for backfill/indexing workloads.
#[cfg(feature = "enterprise")]
const EMBED_BATCH_SIZE: usize = 8;

/// Thread-safe wrapper around fastembed's TextEmbedding.
/// fastembed is sync and CPU-bound, so we wrap in a Mutex and
/// use spawn_blocking to avoid starving the async runtime.
pub struct EmbeddingService {
    #[cfg(feature = "enterprise")]
    model: Arc<Mutex<TextEmbedding>>,
}

impl EmbeddingService {
    /// Initialize the embedding model. This downloads/loads the ONNX model
    /// on first call (~33MB). Call once at startup.
    #[cfg(feature = "enterprise")]
    pub fn new() -> Result<Self, String> {
        let model = TextEmbedding::try_new(
            InitOptions::new(EmbeddingModel::BGESmallENV15).with_show_download_progress(true),
        )
        .map_err(|e| format!("Failed to load embedding model: {e}"))?;
        Ok(Self {
            model: Arc::new(Mutex::new(model)),
        })
    }

    #[cfg(not(feature = "enterprise"))]
    pub fn new() -> Result<Self, String> {
        Err("Embedding service requires enterprise feature".to_string())
    }

    /// Embed a batch of texts. Returns one Vec<f32> per input text.
    /// Each vector has 384 dimensions (bge-small-en-v1.5).
    #[cfg(feature = "enterprise")]
    pub async fn embed(&self, texts: Vec<String>) -> Result<Vec<Vec<f32>>, String> {
        let model = self.model.clone();
        tokio::task::spawn_blocking(move || {
            let model = model.blocking_lock();
            model
                .embed(texts, Some(EMBED_BATCH_SIZE))
                .map_err(|e| format!("Embedding failed: {e}"))
        })
        .await
        .map_err(|e| format!("Spawn blocking failed: {e}"))?
    }

    #[cfg(not(feature = "enterprise"))]
    pub async fn embed(&self, _texts: Vec<String>) -> Result<Vec<Vec<f32>>, String> {
        Err("Embedding service requires enterprise feature".to_string())
    }

    /// Convenience: embed a single text.
    pub async fn embed_one(&self, text: &str) -> Result<Vec<f32>, String> {
        let mut results = self.embed(vec![text.to_string()]).await?;
        results
            .pop()
            .ok_or_else(|| "Empty embedding result".to_string())
    }
}
