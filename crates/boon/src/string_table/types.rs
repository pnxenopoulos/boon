use std::collections::HashMap;

#[derive(Debug, Clone)]
pub struct StringTableEntry {
    pub key: String,
    pub user_data: Vec<u8>,
}

#[derive(Debug, Clone)]
pub struct StringTable {
    pub name: String,
    pub flags: i32,
    // keep sparse; updates can poke holes
    pub entries: Vec<Option<StringTableEntry>>,
}

#[derive(Debug, Default, Clone)]
pub struct StringTables {
    pub by_id: Vec<StringTable>,               // table_id -> table
    pub by_name: HashMap<String, usize>,       // name -> table_id
}

impl StringTables {
    #[inline]
    pub fn get(&self, name: &str) -> Option<&StringTable> {
        self.by_name.get(name).and_then(|&id| self.by_id.get(id))
    }
    #[inline]
    pub fn get_mut(&mut self, name: &str) -> Option<&mut StringTable> {
        if let Some(&id) = self.by_name.get(name) {
            return self.by_id.get_mut(id);
        }
        None
    }
}
