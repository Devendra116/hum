use rand::seq::SliceRandom;

use crate::types::Track;

pub struct Queue {
    tracks: Vec<Track>,
    current_index: Option<usize>,
    pub radio_mode: bool,
}

impl Queue {
    pub fn new() -> Self {
        Self {
            tracks: Vec::new(),
            current_index: None,
            radio_mode: false,
        }
    }

    pub fn add(&mut self, track: Track) {
        self.tracks.push(track);
        if self.current_index.is_none() {
            self.current_index = Some(0);
        }
    }

    pub fn add_many(&mut self, tracks: Vec<Track>) {
        for t in tracks {
            self.tracks.push(t);
        }
        if self.current_index.is_none() && !self.tracks.is_empty() {
            self.current_index = Some(0);
        }
    }

    pub fn current(&self) -> Option<&Track> {
        self.current_index.and_then(|i| self.tracks.get(i))
    }

    pub fn next(&mut self) -> Option<&Track> {
        if let Some(idx) = self.current_index {
            if idx + 1 < self.tracks.len() {
                self.current_index = Some(idx + 1);
                return self.tracks.get(idx + 1);
            }
        }
        None
    }

    pub fn prev(&mut self) -> Option<&Track> {
        if let Some(idx) = self.current_index {
            if idx > 0 {
                self.current_index = Some(idx - 1);
                return self.tracks.get(idx - 1);
            }
        }
        None
    }

    pub fn shuffle(&mut self) {
        let mut rng = rand::thread_rng();
        let current_id = self.current().map(|t| t.id.clone());

        self.tracks.shuffle(&mut rng);

        if let Some(id) = current_id {
            self.current_index = self.tracks.iter().position(|t| t.id == id);
        }
    }

    pub fn clear(&mut self) {
        self.tracks.clear();
        self.current_index = None;
    }

    pub fn remove_current(&mut self) {
        if let Some(idx) = self.current_index {
            if idx < self.tracks.len() {
                self.tracks.remove(idx);
                if self.tracks.is_empty() {
                    self.current_index = None;
                } else if idx >= self.tracks.len() {
                    self.current_index = Some(self.tracks.len() - 1);
                }
            }
        }
    }

    pub fn tracks(&self) -> &[Track] {
        &self.tracks
    }

    pub fn current_index(&self) -> Option<usize> {
        self.current_index
    }

    pub fn len(&self) -> usize {
        self.tracks.len()
    }

    pub fn is_empty(&self) -> bool {
        self.tracks.is_empty()
    }

    pub fn has_next(&self) -> bool {
        self.current_index
            .map(|i| i + 1 < self.tracks.len())
            .unwrap_or(false)
    }

    pub fn set_current(&mut self, index: usize) {
        if index < self.tracks.len() {
            self.current_index = Some(index);
        }
    }
}
