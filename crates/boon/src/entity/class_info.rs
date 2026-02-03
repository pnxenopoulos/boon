use boon_proto::proto::CDemoClassInfo;

/// A single entity class entry.
#[derive(Debug, Clone)]
pub struct ClassEntry {
    pub class_id: i32,
    pub network_name: String,
    pub table_name: String,
}

/// Parsed class info from DEM_ClassInfo. Maps class IDs to network names.
#[derive(Debug, Clone)]
pub struct ClassInfo {
    pub classes: Vec<ClassEntry>,
    /// Number of bits needed to encode a class_id.
    pub bits: usize,
}

impl ClassInfo {
    pub fn parse(cmd: CDemoClassInfo) -> Self {
        let classes: Vec<ClassEntry> = cmd
            .classes
            .into_iter()
            .map(|c| ClassEntry {
                class_id: c.class_id.unwrap_or(0),
                network_name: c.network_name.unwrap_or_default(),
                table_name: c.table_name.unwrap_or_default(),
            })
            .collect();

        let max_id = classes.iter().map(|c| c.class_id).max().unwrap_or(0) as u32;
        let bits = if max_id == 0 {
            1
        } else {
            32 - max_id.leading_zeros() as usize
        };

        ClassInfo { classes, bits }
    }

    pub fn by_id(&self, class_id: i32) -> Option<&ClassEntry> {
        self.classes.iter().find(|c| c.class_id == class_id)
    }

    pub fn name_by_id(&self, class_id: i32) -> Option<&str> {
        self.by_id(class_id).map(|c| c.network_name.as_str())
    }
}
