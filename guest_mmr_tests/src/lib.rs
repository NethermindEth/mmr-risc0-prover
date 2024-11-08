use guest_types::{AppendResult, PeaksFormattingOptions, PeaksOptions};
use serde::{Deserialize, Serialize};
use starknet_crypto::{poseidon_hash, poseidon_hash_many, poseidon_hash_single, Felt};
use std::collections::{HashMap, VecDeque};
use thiserror::Error;

#[derive(Error, Debug)]
pub enum FormattingError {
    #[error("Formatting: Expected peaks output size is smaller than the actual size")]
    PeaksOutputSizeError,
}

#[derive(Error, Debug)]
pub enum MMRError {
    NoHashFoundForIndex(usize),
    Formatting(FormattingError),
    InsufficientPeaksForMerge,
    HashError,
}

pub struct GuestMMR {
    hashes: HashMap<usize, String>,
    elements_count: usize,
    leaves_count: usize,
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
        println!("elements_count: {}", elements_count);

        let mut peaks: Vec<String> = self.retrieve_peaks_hashes(find_peaks(elements_count))?;
        println!("peaks: {:?}", peaks);

        let mut last_element_idx = self.elements_count + 1;
        let leaf_element_index = last_element_idx;
        println!("leaf_element_index: {}", leaf_element_index);

        // Store the new leaf in the hash map
        self.hashes.insert(last_element_idx, value.clone());

        peaks.push(value);

        let no_merges = leaf_count_to_append_no_merges(self.leaves_count);
        println!("no_merges: {}", no_merges);
        for _ in 0..no_merges {
            if peaks.len() < 2 {
                return Err(MMRError::InsufficientPeaksForMerge);
            }

            last_element_idx += 1;

            // Pop the last two peaks to merge
            let right_hash = peaks.pop().unwrap();
            println!("right_hash: {}", right_hash);
            let left_hash = peaks.pop().unwrap();
            println!("left_hash: {}", left_hash);

            let parent_hash = hash(vec![left_hash, right_hash])?;
            println!("parent_hash: {}", parent_hash);

            self.hashes.insert(last_element_idx, parent_hash.clone());

            peaks.push(parent_hash);
        }

        self.elements_count = last_element_idx;
        self.leaves_count += 1;

        let bag = self.bag_the_peaks()?;
        println!("bag: {}", bag);

        let root_hash = self.calculate_root_hash(&bag, last_element_idx)?;
        println!("last_element_idx: {}", last_element_idx);
        println!("root_hash: {}", root_hash);

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
            0 => Ok("0x0".to_string()),
            1 => Ok(peaks_hashes[0].clone()),
            _ => {
                let mut peaks_hashes: VecDeque<String> = peaks_hashes.into();
                let last = peaks_hashes.pop_back().unwrap();
                let second_last = peaks_hashes.pop_back().unwrap();
                let root0 = hash(vec![second_last, last])?;

                let final_root = peaks_hashes
                    .into_iter()
                    .rev()
                    .fold(root0, |prev: String, cur: String| {
                        hash(vec![cur, prev]).unwrap()
                    });

                Ok(final_root)
            }
        }
    }

    pub fn calculate_root_hash(
        &self,
        bag: &str,
        elements_count: usize,
    ) -> Result<String, MMRError> {
        match hash(vec![elements_count.to_string(), bag.to_string()]) {
            Ok(root_hash) => Ok(root_hash),
            Err(_) => Err(MMRError::HashError),
        }
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
pub struct Proof {
    pub element_index: usize,
    pub element_hash: String,
    pub siblings_hashes: Vec<String>,
    pub peaks_hashes: Vec<String>,
    pub elements_count: usize,
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

fn hash(data: Vec<String>) -> Result<String, MMRError> {
    // for element in &data {
    //     self.is_element_size_valid(element)?;
    // }

    let field_elements: Vec<Felt> = data
        .iter()
        .map(|e| Felt::from_hex(e).unwrap_or_default())
        .collect();

    let hash_core = match field_elements.len() {
        0 => return Err(MMRError::HashError),
        1 => poseidon_hash_single(field_elements[0]),
        2 => poseidon_hash(field_elements[0], field_elements[1]),
        _ => poseidon_hash_many(&field_elements),
    };

    let hash = format!("{:x}", hash_core);
    // if self.should_pad {
    //     hash = format!("{:0>63}", hash);
    // }
    let hash = format!("0x{}", hash);
    Ok(hash)
}

#[cfg(test)]
mod tests {
    use super::*;
    use guest_types::{PeaksFormattingOptions, PeaksOptions};

    #[test]
    fn test_guest_mmr_initialization() {
        let initial_peaks = vec!["0xabc".to_string(), "0xdef".to_string()];
        let elements_count = 3;
        let leaves_count = 2;
        let guest_mmr = GuestMMR::new(initial_peaks.clone(), elements_count, leaves_count);

        // Check elements and leaves count
        assert_eq!(guest_mmr.get_elements_count(), elements_count);
        assert_eq!(guest_mmr.get_leaves_count(), leaves_count);

        // Check initial peaks are stored correctly
        let peak_positions = find_peaks(elements_count);
        for (peak, pos) in initial_peaks.iter().zip(peak_positions) {
            assert_eq!(guest_mmr.hashes.get(&pos).unwrap(), peak);
        }
    }

    #[test]
    fn test_guest_mmr_append() {
        // Initialize an empty GuestMMR
        let initial_peaks = vec![];
        let elements_count = 0;
        let leaves_count = 0;
        let mut guest_mmr = GuestMMR::new(initial_peaks, elements_count, leaves_count);

        // Append a value
        let value = "0x123".to_string();
        let append_result = guest_mmr.append(value.clone()).expect("Append failed");
        // Check counts
        assert_eq!(guest_mmr.get_elements_count(), 1);
        assert_eq!(guest_mmr.get_leaves_count(), 1);

        // Check the new element is stored
        assert_eq!(guest_mmr.hashes.get(&1).unwrap(), &value);

        // Verify append result
        assert_eq!(append_result.leaves_count, 1);
        assert_eq!(append_result.elements_count, 1);
        assert_eq!(append_result.element_index, 1);

        // Verify root hash
        let expected_bag = guest_mmr.bag_the_peaks().expect("Bag the peaks failed");
        let expected_root_hash = guest_mmr
            .calculate_root_hash(&expected_bag, guest_mmr.get_elements_count())
            .expect("Calculate root hash failed");
        assert_eq!(append_result.root_hash, expected_root_hash);
    }

    #[test]
    fn test_guest_mmr_get_peaks() {
        // Initialize GuestMMR and append elements
        let initial_peaks = vec![];
        let elements_count = 0;
        let leaves_count = 0;
        let mut guest_mmr = GuestMMR::new(initial_peaks, elements_count, leaves_count);

        guest_mmr
            .append("0x6c17009d66e34c1d6b7e4d73fd5a105243feb10c7cae9598d60b0fa97d08868".to_string())
            .expect("Append failed");
        guest_mmr
            .append("0x4998b07fef69c1b1658fcb44d44fa5bb0ca62c835b26fe763ca14b61a6595da".to_string())
            .expect("Append failed");
        guest_mmr
            .append("0x7337cf1262bf9eeaecffe02776fa1cc9fd35c6fc49303a2b5f39d96a7b46afa".to_string())
            .expect("Append failed");
        guest_mmr
            .append("0x16fa2f065f204a16db293c9adf370da4e08eea45874692dfa00123b21bbfe81".to_string())
            .expect("Append failed");
        // Get peaks
        let peaks_options = PeaksOptions {
            elements_count: None,
            formatting_opts: None,
        };
        let peaks = guest_mmr
            .get_peaks(peaks_options)
            .expect("Get peaks failed");
        println!("peaks: {:?}", peaks);

        // Expected peaks
        let peaks_indices = find_peaks(guest_mmr.get_elements_count());
        let expected_peaks = guest_mmr
            .retrieve_peaks_hashes(peaks_indices)
            .expect("Retrieve peaks hashes failed");

        assert_eq!(peaks, expected_peaks);
    }

    #[test]
    fn test_guest_mmr_bag_the_peaks() {
        // Initialize GuestMMR and append elements
        let initial_peaks = vec![];
        let elements_count = 0;
        let leaves_count = 0;
        let mut guest_mmr = GuestMMR::new(initial_peaks, elements_count, leaves_count);

        guest_mmr
            .append("0x6c17009d66e34c1d6b7e4d73fd5a105243feb10c7cae9598d60b0fa97d08868".to_string())
            .expect("Append failed");
        guest_mmr
            .append("0x4998b07fef69c1b1658fcb44d44fa5bb0ca62c835b26fe763ca14b61a6595da".to_string())
            .expect("Append failed");
        guest_mmr
            .append("0x7337cf1262bf9eeaecffe02776fa1cc9fd35c6fc49303a2b5f39d96a7b46afa".to_string())
            .expect("Append failed");
        guest_mmr
            .append("0x16fa2f065f204a16db293c9adf370da4e08eea45874692dfa00123b21bbfe81".to_string())
            .expect("Append failed");

        // Bag the peaks
        let bag = guest_mmr.bag_the_peaks().expect("Bag the peaks failed");
        println!("bag: {:?}", bag);

        // Calculate root hash
        let root_hash = guest_mmr
            .calculate_root_hash(&bag, guest_mmr.get_elements_count())
            .expect("Calculate root hash failed");
        println!("root_hash: {:?}", root_hash);
        // Verify root hash is not empty
        assert!(!root_hash.is_empty());
    }

    #[test]
    fn test_format_peaks() {
        let peaks = vec!["0x1".to_string(), "0x2".to_string()];
        let formatting_opts = PeaksFormattingOptions {
            output_size: 4,
            null_value: "0x0".to_string(),
        };

        let formatted_peaks =
            format_peaks(peaks.clone(), &formatting_opts).expect("Format peaks failed");

        let expected_peaks = vec![
            "0x1".to_string(),
            "0x2".to_string(),
            "0x0".to_string(),
            "0x0".to_string(),
        ];

        assert_eq!(formatted_peaks, expected_peaks);
    }

    #[test]
    fn test_format_peaks_error() {
        let peaks = vec!["0x1".to_string(), "0x2".to_string(), "0x3".to_string()];
        let formatting_opts = PeaksFormattingOptions {
            output_size: 2,
            null_value: "0x0".to_string(),
        };

        let result = format_peaks(peaks, &formatting_opts);

        assert!(matches!(result, Err(FormattingError::PeaksOutputSizeError)));
    }
}
