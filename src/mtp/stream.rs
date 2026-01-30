//! Streaming download/upload support.

use bytes::Bytes;
use futures::Stream;
use std::pin::Pin;
use std::task::{Context, Poll};

/// A chunk of downloaded data.
#[derive(Debug)]
pub struct DownloadChunk {
    /// The data in this chunk
    pub data: Bytes,
    /// Total bytes received so far
    pub bytes_so_far: u64,
    /// Total file size (if known)
    pub total_bytes: Option<u64>,
}

/// Progress information for transfers.
#[derive(Debug, Clone)]
pub struct Progress {
    /// Bytes transferred so far
    pub bytes_transferred: u64,
    /// Total bytes (if known)
    pub total_bytes: Option<u64>,
}

impl Progress {
    /// Progress as a percentage (0.0 to 100.0), if total is known.
    pub fn percent(&self) -> Option<f64> {
        self.total_bytes.map(|total| {
            if total == 0 {
                100.0
            } else {
                self.bytes_transferred as f64 / total as f64 * 100.0
            }
        })
    }

    /// Progress as a fraction (0.0 to 1.0), if total is known.
    pub fn fraction(&self) -> Option<f64> {
        self.total_bytes.map(|total| {
            if total == 0 {
                1.0
            } else {
                self.bytes_transferred as f64 / total as f64
            }
        })
    }
}

// Note: DownloadStream is complex and requires streaming from PtpSession.
// For now, we'll implement a simpler version that doesn't stream from USB
// but converts downloaded data to a stream.

/// A stream of file chunks during download.
///
/// Implements `Stream<Item = Result<DownloadChunk, Error>>`.
pub struct DownloadStream {
    data: Option<Vec<u8>>,
    total_size: u64,
    chunk_size: usize,
    position: u64,
}

impl DownloadStream {
    /// Create a new download stream from downloaded data.
    pub(crate) fn new(data: Vec<u8>) -> Self {
        let total_size = data.len() as u64;
        Self {
            data: Some(data),
            total_size,
            chunk_size: 64 * 1024, // 64KB chunks
            position: 0,
        }
    }

    /// Total file size.
    pub fn total_size(&self) -> u64 {
        self.total_size
    }

    /// Collect all chunks into a `Vec<u8>`.
    pub async fn collect(self) -> Result<Vec<u8>, crate::Error> {
        Ok(self.data.unwrap_or_default())
    }
}

impl Stream for DownloadStream {
    type Item = Result<DownloadChunk, crate::Error>;

    fn poll_next(mut self: Pin<&mut Self>, _cx: &mut Context<'_>) -> Poll<Option<Self::Item>> {
        let data = match self.data.take() {
            Some(d) => d,
            None => return Poll::Ready(None),
        };

        if self.position >= self.total_size {
            return Poll::Ready(None);
        }

        let start = self.position as usize;
        let end = std::cmp::min(start + self.chunk_size, data.len());
        let chunk_data = Bytes::copy_from_slice(&data[start..end]);

        self.position = end as u64;

        // Put data back if there's more to read
        if self.position < self.total_size {
            self.data = Some(data);
        }

        Poll::Ready(Some(Ok(DownloadChunk {
            data: chunk_data,
            bytes_so_far: self.position,
            total_bytes: Some(self.total_size),
        })))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use futures::StreamExt;

    #[test]
    fn test_progress_percent() {
        let p = Progress {
            bytes_transferred: 50,
            total_bytes: Some(100),
        };
        assert_eq!(p.percent(), Some(50.0));

        let p = Progress {
            bytes_transferred: 100,
            total_bytes: Some(100),
        };
        assert_eq!(p.percent(), Some(100.0));

        let p = Progress {
            bytes_transferred: 0,
            total_bytes: Some(0),
        };
        assert_eq!(p.percent(), Some(100.0)); // Special case: empty file

        let p = Progress {
            bytes_transferred: 50,
            total_bytes: None,
        };
        assert_eq!(p.percent(), None);
    }

    #[test]
    fn test_progress_fraction() {
        let p = Progress {
            bytes_transferred: 50,
            total_bytes: Some(100),
        };
        assert_eq!(p.fraction(), Some(0.5));

        let p = Progress {
            bytes_transferred: 25,
            total_bytes: Some(100),
        };
        assert_eq!(p.fraction(), Some(0.25));

        let p = Progress {
            bytes_transferred: 0,
            total_bytes: Some(0),
        };
        assert_eq!(p.fraction(), Some(1.0)); // Special case: empty file

        let p = Progress {
            bytes_transferred: 50,
            total_bytes: None,
        };
        assert_eq!(p.fraction(), None);
    }

    #[tokio::test]
    async fn test_download_stream() {
        let data = vec![1, 2, 3, 4, 5];
        let stream = DownloadStream::new(data.clone());

        assert_eq!(stream.total_size(), 5);

        let collected = stream.collect().await.unwrap();
        assert_eq!(collected, data);
    }

    #[tokio::test]
    async fn test_download_stream_chunks() {
        // Create data larger than chunk size to test chunking
        let data: Vec<u8> = (0..200_000u32).map(|i| (i % 256) as u8).collect();
        let expected_data = data.clone();
        let mut stream = DownloadStream::new(data);

        let mut chunks = Vec::new();
        while let Some(result) = stream.next().await {
            let chunk = result.unwrap();
            chunks.push(chunk);
        }

        // Should have multiple chunks
        assert!(chunks.len() > 1);

        // Verify bytes_so_far increases
        let mut prev_bytes = 0;
        for chunk in &chunks {
            assert!(chunk.bytes_so_far > prev_bytes);
            prev_bytes = chunk.bytes_so_far;
        }

        // Last chunk should have all bytes
        assert_eq!(chunks.last().unwrap().bytes_so_far, expected_data.len() as u64);

        // Collect all data from chunks and verify
        let collected: Vec<u8> = chunks.iter().flat_map(|c| c.data.iter().copied()).collect();
        assert_eq!(collected, expected_data);
    }

    #[tokio::test]
    async fn test_download_stream_empty() {
        let stream = DownloadStream::new(vec![]);
        assert_eq!(stream.total_size(), 0);
        let collected = stream.collect().await.unwrap();
        assert!(collected.is_empty());
    }

    #[test]
    fn test_progress_edge_cases() {
        // Test with very large numbers
        let p = Progress {
            bytes_transferred: u64::MAX / 2,
            total_bytes: Some(u64::MAX),
        };
        assert!(p.fraction().unwrap() > 0.49 && p.fraction().unwrap() < 0.51);

        // Test 100% progress
        let p = Progress {
            bytes_transferred: 1000,
            total_bytes: Some(1000),
        };
        assert_eq!(p.percent(), Some(100.0));
        assert_eq!(p.fraction(), Some(1.0));
    }

    #[test]
    fn test_download_chunk_debug() {
        let chunk = DownloadChunk {
            data: Bytes::from_static(&[1, 2, 3]),
            bytes_so_far: 3,
            total_bytes: Some(10),
        };
        // Just verify Debug is implemented and doesn't panic
        let _ = format!("{:?}", chunk);
    }

    #[test]
    fn test_progress_debug_and_clone() {
        let p = Progress {
            bytes_transferred: 50,
            total_bytes: Some(100),
        };
        let _ = format!("{:?}", p);
        let p2 = p.clone();
        assert_eq!(p.bytes_transferred, p2.bytes_transferred);
        assert_eq!(p.total_bytes, p2.total_bytes);
    }
}
