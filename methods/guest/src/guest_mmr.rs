// guest_mmr.rs
use std::collections::{HashMap, VecDeque};
// use sha3::Digest;
use serde::{Serialize, Deserialize};

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
        println!("Appending value: {}", value);
        println!("Current elements_count: {}", self.elements_count);
        println!("Current leaves_count: {}", self.leaves_count);

        // Special handling for first element
        if self.elements_count == 0 {
            self.elements_count = 1;
            self.leaves_count = 1;
            self.hashes.insert(1, value.clone());
            
            println!("First element added at index 1");
            return Ok(AppendResult {
                leaves_count: 1,
                elements_count: 1,
                element_index: 1,
                root_hash: value,
            });
        }

        let peaks_indices = find_peaks(self.elements_count);
        println!("Current peak indices: {:?}", peaks_indices);
        
        let mut peaks = self.retrieve_peaks_hashes(peaks_indices)?;
        println!("Current peaks: {:?}", peaks);

        let mut last_element_idx = self.elements_count + 1;
        let leaf_element_index = last_element_idx;

        // Store the new hash
        self.hashes.insert(last_element_idx, value.clone());
        peaks.push(value);

        let no_merges = leaf_count_to_append_no_merges(self.leaves_count);
        println!("Number of merges needed: {}", no_merges);

        for merge_idx in 0..no_merges {
            if peaks.len() < 2 {
                println!("Not enough peaks for merge {}", merge_idx);
                break;
            }
            
            let right_hash = peaks.pop()
                .ok_or_else(|| MMRError::NoHashFoundForIndex(last_element_idx))?;
            
            let left_hash = peaks.pop()
                .ok_or_else(|| MMRError::NoHashFoundForIndex(last_element_idx))?;

            println!("Merging: left={}, right={}", left_hash, right_hash);
            let parent_hash = hash_pair(&left_hash, &right_hash)
                .map_err(|e| MMRError::HashingError(e.to_string()))?;
            
            last_element_idx += 1;
            println!("Storing parent hash at index {}: {}", last_element_idx, parent_hash);
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
        peak_idxs.iter()
            .map(|&idx| {
                self.hashes.get(&idx)
                    .cloned()
                    .ok_or(MMRError::NoHashFoundForIndex(idx))
            })
            .collect()
    }

    fn bag_the_peaks(&self) -> Result<String, MMRError> {
        let peaks_idxs = find_peaks(self.elements_count);
        if peaks_idxs.is_empty() {
            return Ok("0x0".to_string());
        }

        let peaks_hashes = self.retrieve_peaks_hashes(peaks_idxs)?;
        
        match peaks_hashes.len() {
            0 => Ok("0x0".to_string()),
            1 => Ok(peaks_hashes[0].clone()),
            _ => {
                let mut peaks: VecDeque<String> = peaks_hashes.into();
                let last = peaks.pop_back().unwrap();
                let second_last = peaks.pop_back().unwrap();
                let root0 = hash_pair(&second_last, &last)?;

                Ok(peaks.into_iter().rev().fold(root0, |prev, cur| {
                    hash_pair(&cur, &prev).unwrap_or(prev)
                }))
            }
        }
    }

    fn calculate_root_hash(&self, bag: &str, elements_count: usize) -> Result<String, MMRError> {
        hash_pair(&elements_count.to_string(), bag)
    }

    pub fn get_peaks(&self) -> Result<Vec<String>, MMRError> {
        let peaks_idxs = find_peaks(self.elements_count);
        self.retrieve_peaks_hashes(peaks_idxs)
    }

    // pub fn verify_proof(
    //     &self,
    //     proof: Proof,
    //     element_value: String,
    // ) -> Result<bool, MMRError> {
    //     let tree_size = self.elements_count;
    //     let leaf_count = mmr_size_to_leaf_count(tree_size);
    //     let peaks_count = leaf_count_to_peaks_count(leaf_count);

    //     if peaks_count != proof.peaks_hashes.len() {
    //         return Err(MMRError::InvalidPeaksCount);
    //     }

    //     let element_index = proof.element_index;

    //     if element_index == 0 || element_index > tree_size {
    //         return Err(MMRError::InvalidElementIndex);
    //     }

    //     let (peak_index, peak_height) = get_peak_info(tree_size, element_index);
    //     if proof.siblings_hashes.len() != peak_height {
    //         return Ok(false);
    //     }

    //     let mut hash = element_value;
    //     let mut leaf_index = element_index_to_leaf_index(element_index)?;

    //     for proof_hash in proof.siblings_hashes.iter() {
    //         let is_right = leaf_index % 2 == 1;
    //         leaf_index /= 2;

    //         hash = if is_right {
    //             hash_pair(proof_hash, &hash)?
    //         } else {
    //             hash_pair(&hash, proof_hash)?
    //         };
    //     }

    //     let peak_hashes = self.get_peaks()?;
    //     Ok(peak_hashes[peak_index] == hash)
    // }
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

#[derive(Debug)]
pub enum MMRError {
    NoHashFoundForIndex(usize),
    HashingError(String),
}

impl std::fmt::Display for MMRError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            MMRError::NoHashFoundForIndex(idx) => write!(f, "No hash found for index {}", idx),
            MMRError::HashingError(msg) => write!(f, "Hashing error: {}", msg),
        }
    }
}

// Helper functions from your original implementation
fn find_peaks(elements_count: usize) -> Vec<usize> {
    if elements_count == 0 {
        return vec![];
    }

    let mut peaks = Vec::new();
    let mut pos = 0;
    let mut size = elements_count;

    while size > 0 {
        let peak_pos = size - (size & size.wrapping_neg());
        if peak_pos > 0 {
            peaks.push(pos + peak_pos);
            pos += peak_pos;
            size -= peak_pos;
        } else {
            break;
        }
    }
    peaks
}

fn hash_pair(left: &str, right: &str) -> Result<String, MMRError> {
    use sha3::{Digest, Keccak256};
    
    let mut hasher = Keccak256::new();
    hasher.update(left.as_bytes());
    hasher.update(right.as_bytes());
    
    Ok(format!("0x{}", hex::encode(hasher.finalize())))
}

// fn mmr_size_to_leaf_count(size: usize) -> usize {
//     let mut count = 0;
//     let mut pos = 0;
//     while pos < size {
//         pos += 1;
//         let height = trailing_zeros(pos);
//         if height == 0 {
//             count += 1;
//         }
//         pos += height;
//     }
//     count
// }

// pub fn leaf_count_to_peaks_count(leaf_count: usize) -> usize {
//     if leaf_count == 0 {
//         return 0;
//     }
//     leaf_count.count_ones() as usize
// }

fn leaf_count_to_append_no_merges(leaf_count: usize) -> usize {
    if leaf_count == 0 {
        return 0;
    }
    (!leaf_count).trailing_zeros() as usize
}

// pub fn element_index_to_leaf_index(element_index: usize) -> Result<usize, MMRError> {
//     if element_index == 0 {
//         return Err(MMRError::InvalidElementIndex);
//     }
    
//     let mut pos = 0;
//     let mut leaf_index = 0;
    
//     while pos < element_index {
//         pos += 1;
//         let height = trailing_zeros(pos);
//         if height == 0 {
//             if pos == element_index {
//                 return Ok(leaf_index);
//             }
//             leaf_index += 1;
//         }
//         pos += height;
//     }
    
//     Err(MMRError::InvalidElementIndex)
// }

// fn get_peak_info(tree_size: usize, element_index: usize) -> (usize, usize) {
//     let peaks = find_peaks(tree_size);
//     let mut peak_index = 0;
//     let mut peak_height = 0;
    
//     for (i, &peak) in peaks.iter().enumerate() {
//         if peak >= element_index {
//             peak_index = i;
//             peak_height = calculate_peak_height(element_index);
//             break;
//         }
//     }
    
//     (peak_index, peak_height)
// }

// pub fn calculate_peak_height(index: usize) -> usize {
//     trailing_zeros(index)
// }

// fn trailing_zeros(mut x: usize) -> usize {
//     if x == 0 {
//         return 0;
//     }
    
//     let mut count = 0;
//     while x & 1 == 0 {
//         count += 1;
//         x >>= 1;
//     }
//     count
// }

// pub fn find_siblings(element_index: usize, tree_size: usize) -> Result<Vec<usize>, MMRError> {
//     if element_index == 0 || element_index > tree_size {
//         return Err(MMRError::InvalidElementIndex);
//     }

//     let mut siblings = Vec::new();
//     let mut pos = element_index;
//     let mut height = 0;
    
//     while pos <= tree_size {
//         let (peak_idx, _) = get_peak_info(tree_size, pos);
//         if peak_idx < find_peaks(tree_size).len() {
//             break;
//         }
        
//         let sibling_pos = if pos % 2 == 0 { pos + 1 } else { pos - 1 };
//         if sibling_pos <= tree_size {
//             siblings.push(sibling_pos);
//         }
        
//         pos = (pos + 1) / 2 + height;
//         height += 1;
//     }
    
//     Ok(siblings)
// }