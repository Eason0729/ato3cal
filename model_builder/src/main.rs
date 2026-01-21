use std::error::Error;
use std::fs::File;
use std::io::BufWriter;
use serde::{Serialize, Deserialize};

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct LinearModel {
    pub slope: f64,
    pub intercept: f64,
}

impl LinearModel {
    pub fn predict(&self, x: f64) -> f64 {
        self.slope * x + self.intercept
    }
}

#[derive(Serialize, Deserialize, Debug)]
pub struct PredictionSystem {
    pub stopover_same: LinearModel,
    pub direct_same: LinearModel,
    pub stopover_twice: LinearModel,
    pub direct_twice: LinearModel,
    pub stopover_thrice: LinearModel,
    pub direct_thrice: LinearModel,
}

fn train_linear_regression(x: &[f64], y: &[f64]) -> LinearModel {
    let n = x.len() as f64;
    let sum_x: f64 = x.iter().sum();
    let sum_y: f64 = y.iter().sum();
    let sum_xy: f64 = x.iter().zip(y.iter()).map(|(xi, yi)| xi * yi).sum();
    let sum_xx: f64 = x.iter().map(|xi| xi * xi).sum();

    let slope = (n * sum_xy - sum_x * sum_y) / (n * sum_xx - sum_x * sum_x);
    let intercept = (sum_y - slope * sum_x) / n;

    LinearModel { slope, intercept }
}

fn main() -> Result<(), Box<dyn Error>> {
    let file_path = "../data.csv";
    let file = File::open(file_path)?;
    let mut rdr = csv::ReaderBuilder::new()
        .has_headers(true)
        .from_reader(file);

    let mut seats = Vec::new();
    let mut y1 = Vec::new(); // StopoverSame
    let mut y2 = Vec::new(); // DirectSame
    let mut y3 = Vec::new(); // StopoverTwice
    let mut y4 = Vec::new(); // DirectTwice
    let mut y5 = Vec::new(); // StopoverThrice
    let mut y6 = Vec::new(); // DirectThrice

    for result in rdr.records() {
        let record = result?;
        // Index 1 is Seats.
        // Indices 2-7 are the targets.
        // Note: record indexing depends on how csv parses the empty first column.
        // If header has one less column or empty string, let's check.
        // We will assume standard CSV behavior.
        
        let s: f64 = record[1].parse()?;
        seats.push(s);
        y1.push(record[2].parse()?);
        y2.push(record[3].parse()?);
        y3.push(record[4].parse()?);
        y4.push(record[5].parse()?);
        y5.push(record[6].parse()?);
        y6.push(record[7].parse()?);
    }

    let sys = PredictionSystem {
        stopover_same: train_linear_regression(&seats, &y1),
        direct_same: train_linear_regression(&seats, &y2),
        stopover_twice: train_linear_regression(&seats, &y3),
        direct_twice: train_linear_regression(&seats, &y4),
        stopover_thrice: train_linear_regression(&seats, &y5),
        direct_thrice: train_linear_regression(&seats, &y6),
    };

    println!("Trained Models: {:?}", sys);

    let out_file = File::create("../model.bin")?;
    let mut writer = BufWriter::new(out_file);
    bincode::serialize_into(&mut writer, &sys)?;
    println!("Model saved to ../model.bin");

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_linear_regression_perfect() {
        let x = vec![1.0, 2.0, 3.0];
        let y = vec![2.0, 4.0, 6.0]; // y = 2x
        let model = train_linear_regression(&x, &y);
        assert!((model.slope - 2.0).abs() < 1e-6);
        assert!((model.intercept - 0.0).abs() < 1e-6);
        assert!((model.predict(4.0) - 8.0).abs() < 1e-6);
    }

    #[test]
    fn test_linear_regression_offset() {
        let x = vec![1.0, 2.0, 3.0];
        let y = vec![3.0, 5.0, 7.0]; // y = 2x + 1
        let model = train_linear_regression(&x, &y);
        assert!((model.slope - 2.0).abs() < 1e-6);
        assert!((model.intercept - 1.0).abs() < 1e-6);
    }
}