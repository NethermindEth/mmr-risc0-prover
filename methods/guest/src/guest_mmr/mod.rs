// guest_mmr.rs
use std::collections::{HashMap, VecDeque};
// use sha3::Digest;
use serde::{Serialize, Deserialize};
use thiserror::Error;
#[derive(Debug, Serialize, Deserialize)]
pub struct GuestInput {
    pub initial_peaks: Vec<String>,
    pub elements_count: usize,
    pub leaves_count: usize,
    pub new_elements: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct GuestOutput {
    pub final_peaks: Vec<String>,
    pub elements_count: usize,
    pub leaves_count: usize,
    pub append_results: Vec<AppendResult>,
}

pub struct GuestMMR {
    hashes: HashMap<usize, String>,
    elements_count: usize,
    leaves_count: usize,
}

#[derive(Clone, Default)]
pub struct PeaksOptions {
    pub elements_count: Option<usize>,
    pub formatting_opts: Option<PeaksFormattingOptions>,
}

#[derive(Clone)]
pub struct FormattingOptions {
    pub output_size: usize,
    pub null_value: String,
}

pub type PeaksFormattingOptions = FormattingOptions;

#[derive(Error, Debug)]
pub enum FormattingError {
    #[error("Formatting: Expected peaks output size is smaller than the actual size")]
    PeaksOutputSizeError,
}

impl GuestMMR {
    pub fn new(initial_peaks: Vec<String>, elements_count: usize, leaves_count: usize) -> Self {
        let mut hashes = HashMap::new();
        
        // Initialize hashes with the peaks at their correct positions
        let peak_positions = find_peaks(elements_count);
        for (peak, pos) in initial_peaks.into_iter().zip(peak_positions) {
            hashes.insert(pos, peak);
        }
        
        Self {
            elements_count,
            leaves_count,
            hashes,
        }
    }

    pub fn get_elements_count(&self) -> usize {
        self.elements_count
    }

    pub fn get_leaves_count(&self) -> usize {
        self.leaves_count
    }

    pub fn append(&mut self, value: String) -> Result<AppendResult, MMRError> {
        let elements_count = self.elements_count;
    
        let mut peaks = self.retrieve_peaks_hashes(find_peaks(elements_count))?;
    
        let mut last_element_idx = self.elements_count + 1;
        let leaf_element_index = last_element_idx;
    
        // Store the new leaf in the hash map
        self.hashes.insert(last_element_idx, value.clone());

        peaks.push(value);
    
        let no_merges = leaf_count_to_append_no_merges(self.leaves_count);
    
        for _ in 0..no_merges {
            if peaks.len() < 2 {
                return Err(MMRError::InsufficientPeaksForMerge);
            }
    
            // Pop the last two peaks to merge
            let right_hash = peaks.pop().unwrap();
            let left_hash = peaks.pop().unwrap();
    
            let parent_hash = hash_pair(&left_hash, &right_hash)?;
            last_element_idx += 1;
            self.hashes.insert(last_element_idx, parent_hash.clone());
    
            peaks.push(parent_hash);
        }
    
        self.elements_count = last_element_idx;
        self.leaves_count += 1;
    
        let bag = self.bag_the_peaks()?;
        let root_hash = self.calculate_root_hash(&bag, last_element_idx)?;
    
        Ok(AppendResult {
            leaves_count: self.leaves_count,
            elements_count: last_element_idx,
            element_index: leaf_element_index,
            root_hash,
        })
    }
    
    
    fn retrieve_peaks_hashes(&self, peak_idxs: Vec<usize>) -> Result<Vec<String>, MMRError> {
        let mut peaks = Vec::new();
    
        for &idx in &peak_idxs {
            // Use `idx` directly since `self.hashes` expects a `usize` key
            if let Some(hash) = self.hashes.get(&idx) {
                peaks.push(hash.clone());
            } else {
                return Err(MMRError::NoHashFoundForIndex(idx));
            }
        }
    
        Ok(peaks)
    }
    
    
    
    fn bag_the_peaks(&self) -> Result<String, MMRError> {
        let peaks_idxs = find_peaks(self.elements_count);
    
        let peaks_hashes = self.retrieve_peaks_hashes(peaks_idxs)?;
    
        match peaks_hashes.len() {
            0 => {
                Ok("0x0".to_string())
            }
            1 => {
                Ok(peaks_hashes[0].clone())
            }
            _ => {
                let mut peaks_hashes: VecDeque<String> = peaks_hashes.into();
                let last = peaks_hashes.pop_back().unwrap();
                let second_last = peaks_hashes.pop_back().unwrap();
                let root0 = hash_pair(&second_last, &last)?;
    
                let final_root = peaks_hashes.into_iter().rev().fold(root0, |prev, cur| {
                    let new_root = hash_pair(&cur, &prev).unwrap_or_else(|_| prev.clone());
                    new_root
                });
                               
                Ok(final_root)
            }
        }
    }
    
    

    fn calculate_root_hash(&self, bag: &str, elements_count: usize) -> Result<String, MMRError> {
        hash_pair(&elements_count.to_string(), bag)
    }

   
    pub fn get_peaks(&self, option: PeaksOptions) -> Result<Vec<String>, MMRError> {
        let tree_size = match option.elements_count {
            Some(count) => count,
            None => self.elements_count,
        };
    
        let peaks_indices = find_peaks(tree_size);
        let peaks = self.retrieve_peaks_hashes(peaks_indices)?;
    
        if let Some(formatting_opts) = option.formatting_opts {
            match format_peaks(peaks, &formatting_opts) {
                Ok(formatted_peaks) => Ok(formatted_peaks),
                Err(e) => Err(MMRError::Formatting(e)),
            }
        } else {
            Ok(peaks)
        }
    }
    
}

#[derive(Debug, Serialize, Deserialize)]
pub struct AppendResult {
    pub leaves_count: usize,
    pub elements_count: usize,
    pub element_index: usize,
    pub root_hash: String,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct Proof {
    pub element_index: usize,
    pub element_hash: String,
    pub siblings_hashes: Vec<String>,
    pub peaks_hashes: Vec<String>,
    pub elements_count: usize,
}

#[derive(Error, Debug)]
pub enum MMRError {
    NoHashFoundForIndex(usize),
    Formatting(FormattingError),
    InsufficientPeaksForMerge,
    HashError,
}

impl std::fmt::Display for MMRError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            MMRError::NoHashFoundForIndex(idx) => write!(f, "No hash found for index {}", idx),
            MMRError::Formatting(e) => write!(f, "Formatting error: {}", e),
            MMRError::InsufficientPeaksForMerge => write!(f, "Insufficient peaks for merge"),
            MMRError::HashError => write!(f, "Hash error"),
        }
    }
}

pub fn format_peaks(
    mut peaks: Vec<String>,
    formatting_opts: &PeaksFormattingOptions,
) -> Result<Vec<String>, FormattingError> {
    if peaks.len() > formatting_opts.output_size {
        return Err(FormattingError::PeaksOutputSizeError);
    }

    let expected_peaks_size_remainder = formatting_opts.output_size - peaks.len();
    let peaks_null_values: Vec<String> =
        vec![formatting_opts.null_value.clone(); expected_peaks_size_remainder];

    peaks.extend(peaks_null_values);

    Ok(peaks)
}

fn hash_pair(left: &str, right: &str) -> Result<String, MMRError> {
    use starknet_crypto::{poseidon_hash, Felt};

    // Remove "0x" prefix if present
    let left_str = left.trim_start_matches("0x");
    let right_str = right.trim_start_matches("0x");

    // Parse the strings into Felt instances
    let left_felt = Felt::from_hex(left_str).map_err(|_| MMRError::HashError)?;
    let right_felt = Felt::from_hex(right_str).map_err(|_| MMRError::HashError)?;

    // Compute the Poseidon hash
    let hash = poseidon_hash(left_felt, right_felt);

    // Return the hash as a hex string (already includes "0x")
    Ok(hash.to_hex_string())
}



// Add this function at the bottom with other helper functions
pub fn find_peaks(mut elements_count: usize) -> Vec<usize> {
    let mut mountain_elements_count = (1 << bit_length(elements_count)) - 1;
    let mut mountain_index_shift = 0;
    let mut peaks = Vec::new();

    while mountain_elements_count > 0 {
        if mountain_elements_count <= elements_count {
            mountain_index_shift += mountain_elements_count;
            peaks.push(mountain_index_shift);
            elements_count -= mountain_elements_count;
        }
        mountain_elements_count >>= 1;
    }

    if elements_count > 0 {
        return Vec::new();
    }

    peaks
}

fn bit_length(num: usize) -> usize {
    (std::mem::size_of::<usize>() * 8) - num.leading_zeros() as usize
}


fn leaf_count_to_append_no_merges(leaf_count: usize) -> usize {
    if leaf_count == 0 {
        return 0;
    }
    (!leaf_count).trailing_zeros() as usize
}

