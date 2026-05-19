use std::collections::HashMap;
use std::path::PathBuf;

pub const DEFAULT_GROUP_ID: u32 = 0;
pub const DEFAULT_GROUP_NAME: &str = "Default";

#[derive(Clone, Debug)]
pub struct GroupTab {
    pub id: u32,
    pub name: String,
}

#[derive(Clone)]
pub struct GroupDragPayload {
    pub source_group_id: u32,
    pub paths: Vec<PathBuf>,
}


#[derive(Clone)]
pub struct GroupPlaylistState {
    pub source_playlist: Vec<PathBuf>,
    pub active_playlist: Vec<PathBuf>,
    pub current_index: usize,
    pub filter: crate::state::FilterState,
}

impl GroupPlaylistState {
    pub fn new() -> Self {
        Self {
            source_playlist: Vec::new(),
            active_playlist: Vec::new(),
            current_index: 0,
            filter: crate::state::FilterState::default(),
        }
    }

    pub fn from_view(view: &crate::state::ViewerState) -> Self {
        Self {
            source_playlist: view.source_playlist.clone(),
            active_playlist: view.active_playlist.clone(),
            current_index: view.current_index,
            filter: view.filter.clone(),
        }
    }

    pub fn apply_to_view(&self, view: &mut crate::state::ViewerState) {
        view.source_playlist = self.source_playlist.clone();
        view.active_playlist = self.active_playlist.clone();
        view.filter = self.filter.clone();
        view.current_index = clamp_index(self.current_index, view.active_playlist.len());
    }

    pub fn rebuild_active_playlist(&mut self) {
        self.active_playlist = crate::playlist_view::build_active_playlist(
            &self.source_playlist,
            &self.filter.criteria,
        );
        self.current_index = clamp_index(self.current_index, self.active_playlist.len());
    }

    pub fn add_items(&mut self, paths: &[PathBuf]) -> usize {
        let mut added = 0;

        for path in paths {
            if self.source_playlist.iter().any(|existing| existing == path) {
                continue;
            }
            self.source_playlist.push(path.clone());
            added += 1;
        }

        added
    }

    pub fn remove_items(&mut self, paths: &[PathBuf]) -> usize {
        let before = self.source_playlist.len();
        self.source_playlist.retain(|path| !paths.iter().any(|candidate| candidate == path));
        before.saturating_sub(self.source_playlist.len())
    }
}

pub struct GroupTabsState {
    pub user_groups: Vec<GroupTab>,
    pub selected_id: u32,
    pub playlists: HashMap<u32, GroupPlaylistState>,
    next_group_number: u32,
    mru: Vec<u32>,
}

impl GroupTabsState {
    pub fn new() -> Self {
        let mut playlists = HashMap::new();
        playlists.insert(DEFAULT_GROUP_ID, GroupPlaylistState::new());
        Self {
            user_groups: Vec::new(),
            selected_id: DEFAULT_GROUP_ID,
            playlists,
            next_group_number: 1,
            mru: vec![DEFAULT_GROUP_ID],
        }
    }

    pub fn reset_for_new_folder(&mut self) {
        self.user_groups.clear();
        self.selected_id = DEFAULT_GROUP_ID;
        self.playlists.clear();
        self.playlists
            .insert(DEFAULT_GROUP_ID, GroupPlaylistState::new());
        self.next_group_number = 1;
        self.mru.clear();
        self.mru.push(DEFAULT_GROUP_ID);
    }

    pub fn add_group(&mut self) -> u32 {
        let id = self.next_group_number;
        self.next_group_number += 1;

        let name = format!("Group {}", id);
        self.user_groups.push(GroupTab { id, name });
        self.playlists.entry(id).or_insert_with(GroupPlaylistState::new);

        self.mru.retain(|&value| value != id);
        self.mru.push(id);

        id
    }

    pub fn select_group(&mut self, id: u32) {
        if id != DEFAULT_GROUP_ID
            && !self.user_groups.iter().any(|group| group.id == id)
        {
            return;
        }

        self.selected_id = id;
        self.bump_mru(id);
    }

    pub fn close_group(&mut self, id: u32) {
        if id == DEFAULT_GROUP_ID {
            return;
        }

        self.user_groups.retain(|group| group.id != id);
        self.playlists.remove(&id);
        self.mru.retain(|&value| value != id);

        if self.selected_id == id {
            self.ensure_default_in_mru();
            let next_id = self.mru.first().copied().unwrap_or(DEFAULT_GROUP_ID);
            self.selected_id = next_id;
            self.bump_mru(next_id);
        }

        if self.user_groups.is_empty() {
            self.next_group_number = 1;
        }
    }

    pub fn is_selected(&self, id: u32) -> bool {
        self.selected_id == id
    }

    pub fn ensure_group_playlist(&mut self, id: u32) {
        self.playlists.entry(id).or_insert_with(GroupPlaylistState::new);
    }

    pub fn set_group_playlist(&mut self, id: u32, state: GroupPlaylistState) {
        self.playlists.insert(id, state);
    }

    pub fn group_playlist(&self, id: u32) -> Option<&GroupPlaylistState> {
        self.playlists.get(&id)
    }

    pub fn group_playlist_mut(&mut self, id: u32) -> Option<&mut GroupPlaylistState> {
        self.playlists.get_mut(&id)
    }

    fn ensure_default_in_mru(&mut self) {
        if !self.mru.iter().any(|&value| value == DEFAULT_GROUP_ID) {
            self.mru.push(DEFAULT_GROUP_ID);
        }
    }

    fn bump_mru(&mut self, id: u32) {
        self.mru.retain(|&value| value != id);
        self.mru.insert(0, id);
    }
}

fn clamp_index(index: usize, len: usize) -> usize {
    if len == 0 {
        0
    } else {
        index.min(len - 1)
    }
}
