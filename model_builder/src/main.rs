use std::error::Error;
use std::fs::File;
use std::io::BufWriter;
use serde::{Serialize, Deserialize};
use nalgebra::{DMatrix, DVector};

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct PolyModel {
    pub weights: Vec<f64>,
}

impl PolyModel {
    // Features: [1.0, Seats, Ratio, Ratio^2, IsDirect]
    pub fn predict(&self, seats: f64, ratio: f64, is_direct: bool) -> f64 {
        let direct_val = if is_direct { 1.0 } else { 0.0 };
        let features = vec![1.0, seats, ratio, ratio * ratio, direct_val];
        
        features.iter().zip(&self.weights).map(|(f, w)| f * w).sum()
    }
}

fn train_model(samples: &[(f64, f64, bool, f64)]) -> PolyModel {
    // samples: (seats, ratio, is_direct, target_sum)
    let n = samples.len();
    let m = 5; // Bias, Seats, Ratio, Ratio^2, IsDirect

    let mut x_vals = Vec::with_capacity(n * m);
    let mut y_vals = Vec::with_capacity(n);

    for (seats, ratio, is_direct, target) in samples {
        x_vals.push(1.0);
        x_vals.push(*seats);
        x_vals.push(*ratio);
        x_vals.push(ratio * ratio);
        x_vals.push(if *is_direct { 1.0 } else { 0.0 });
        
        y_vals.push(*target);
    }

    let x = DMatrix::from_row_slice(n, m, &x_vals);
    let y = DVector::from_column_slice(&y_vals);

    // Solve (X^T * X)^-1 * X^T * Y
    // Using SVD decomposition for stability: OLS
    let ols = x.svd(true, true).solve(&y, 1e-10).expect("Linear regression failed");
    
    let weights: Vec<f64> = ols.iter().cloned().collect();

    PolyModel { weights }
}

fn main() -> Result<(), Box<dyn Error>> {
    let file_path = "../data.csv";
    let file = File::open(file_path)?;
    let mut rdr = csv::ReaderBuilder::new()
        .has_headers(true)
        .from_reader(file);

    let mut samples = Vec::new();

    for result in rdr.records() {
        let record = result?;
        let seats: f64 = record[1].parse()?;
        
        // CSV Cols: 
        // 2: Stopover 1.0
        // 3: Direct 1.0
        // 4: Stopover 2.0
        // 5: Direct 2.0
        // 6: Stopover 3.0
        // 7: Direct 3.0

        // Ratio 1.0
        samples.push((seats, 1.0, false, record[2].parse::<f64>()?));
        samples.push((seats, 1.0, true, record[3].parse::<f64>()?));

        // Ratio 2.0
        samples.push((seats, 2.0, false, record[4].parse::<f64>()?));
        samples.push((seats, 2.0, true, record[5].parse::<f64>()?));

        // Ratio 3.0
        samples.push((seats, 3.0, false, record[6].parse::<f64>()?));
        samples.push((seats, 3.0, true, record[7].parse::<f64>()?));
    }

    let model = train_model(&samples);
    println!("Trained Weights: {:?}", model.weights);

    let out_file = File::create("../model.bin")?;
    let mut writer = BufWriter::new(out_file);
    bincode::serialize_into(&mut writer, &model)?;
    println!("Model saved to ../model.bin");

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_model_monotonicity_ratio() {
        // Create a dummy model (or train on small data)
        // Let's train on a tiny subset that mimics the real rule: 
        // Base=1000, +50 for Ratio 2, +150 for Ratio 3.
        let samples = vec![
            (100.0, 1.0, false, 1000.0),
            (100.0, 2.0, false, 1050.0),
            (100.0, 3.0, false, 1150.0),
        ];
        
        let model = train_model(&samples);
        
        let p1 = model.predict(100.0, 1.0, false);
        let p2 = model.predict(100.0, 2.0, false);
        let p3 = model.predict(100.0, 3.0, false);
        
        // Verify increasing ratio increases cost
        assert!(p2 > p1);
        assert!(p3 > p2);
        
        // Verify non-linear jump (1.0->2.0 is +50, 2.0->3.0 is +100)
        let diff1 = p2 - p1;
        let diff2 = p3 - p2;
        assert!((diff2 - diff1).abs() > 10.0); // Expect acceleration
    }
}
