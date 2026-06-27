use std::fs::File;
use std::io::BufWriter;

use polars::io::parquet::write::BatchedWriter as ParquetBatchedWriter;
use polars::prelude::*;

use crate::Result;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ComputeSummary {
    pub output_path: String,
    pub n_observations: usize,
    pub n_rows: usize,
}

#[derive(Debug)]
pub enum ComputeResult {
    Memory(DataFrame),
    File(ComputeSummary),
}

pub enum ComputeSink {
    Memory(MemorySink),
    File(FileSink),
}

impl ComputeSink {
    pub fn for_output(output_path: Option<&str>) -> Self {
        match output_path {
            Some(path) => Self::File(FileSink::new(path.to_string())),
            None => Self::Memory(MemorySink::new()),
        }
    }

    pub fn write_observation(&mut self, df: DataFrame) -> Result<()> {
        match self {
            Self::Memory(sink) => sink.write_observation(df),
            Self::File(sink) => sink.write_observation(df),
        }
    }

    pub fn finish(self) -> Result<ComputeResult> {
        match self {
            Self::Memory(sink) => Ok(ComputeResult::Memory(sink.finish()?)),
            Self::File(sink) => Ok(ComputeResult::File(sink.finish()?)),
        }
    }
}

#[derive(Debug, Default)]
pub struct MemorySink {
    frames: Vec<DataFrame>,
}

impl MemorySink {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn write_observation(&mut self, df: DataFrame) -> Result<()> {
        self.frames.push(df);
        Ok(())
    }

    pub fn finish(self) -> Result<DataFrame> {
        let mut frames = self.frames.into_iter();
        let Some(mut out) = frames.next() else {
            return Ok(DataFrame::default());
        };

        for frame in frames {
            out.vstack_mut(&frame)?;
        }

        Ok(out)
    }
}

pub struct FileSink {
    output_path: String,
    writer: Option<ParquetBatchedWriter<BufWriter<File>>>,
    n_observations: usize,
    n_rows: usize,
}

impl FileSink {
    pub fn new(output_path: String) -> Self {
        Self {
            output_path,
            writer: None,
            n_observations: 0,
            n_rows: 0,
        }
    }

    pub fn write_observation(&mut self, mut df: DataFrame) -> Result<()> {
        df.align_chunks();
        if self.writer.is_none() {
            let file = File::create(&self.output_path)?;
            let writer = ParquetWriter::new(BufWriter::new(file)).batched(df.schema())?;
            self.writer = Some(writer);
        }

        self.writer
            .as_mut()
            .expect("writer initialized above")
            .write_batch(&df)?;
        self.n_observations += 1;
        self.n_rows += df.height();
        Ok(())
    }

    pub fn finish(self) -> Result<ComputeSummary> {
        if let Some(writer) = self.writer {
            writer.finish()?;
        }

        Ok(ComputeSummary {
            output_path: self.output_path,
            n_observations: self.n_observations,
            n_rows: self.n_rows,
        })
    }
}

#[cfg(test)]
mod tests {
    use std::fs::File;

    use super::*;

    #[test]
    fn memory_sink_stacks_observations() -> Result<()> {
        let mut sink = MemorySink::new();
        sink.write_observation(df!("time" => [1i64], "ret" => [0.1])?)?;
        sink.write_observation(df!("time" => [2i64], "ret" => [0.2])?)?;

        let out = sink.finish()?;
        assert_eq!(out.height(), 2);
        assert_eq!(
            out.column("ret")?
                .try_f64()
                .expect("ret is f64")
                .into_no_null_iter()
                .collect::<Vec<_>>(),
            [0.1, 0.2]
        );
        Ok(())
    }

    #[test]
    fn file_sink_writes_parquet_batches() -> Result<()> {
        let path = std::env::temp_dir().join(format!(
            "qfactors-file-sink-{}-{}.parquet",
            std::process::id(),
            std::thread::current().name().unwrap_or("test")
        ));
        let path_string = path.to_string_lossy().to_string();
        let mut sink = FileSink::new(path_string.clone());

        sink.write_observation(df!("time" => [1i64], "ret" => [0.1])?)?;
        sink.write_observation(df!("time" => [2i64], "ret" => [0.2])?)?;
        let summary = sink.finish()?;

        assert_eq!(summary.n_observations, 2);
        assert_eq!(summary.n_rows, 2);

        let out = ParquetReader::new(File::open(&path)?).finish()?;
        assert_eq!(
            out.column("ret")?
                .try_f64()
                .expect("ret is f64")
                .into_no_null_iter()
                .collect::<Vec<_>>(),
            [0.1, 0.2]
        );

        let _ = std::fs::remove_file(path);
        Ok(())
    }
}
