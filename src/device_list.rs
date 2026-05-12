use crate::tag_list::TagDefList;
use std::collections::{BTreeMap, btree_map};

pub struct DeviceDef {
    pub addr: u8, // Device or unit address
    pub cyclic_write: bool,
    pub tags: TagDefList,
}

pub struct DeviceDefList(BTreeMap<u8, DeviceDef>);

impl DeviceDefList {
    pub fn new() -> DeviceDefList {
        DeviceDefList(BTreeMap::new())
    }

    pub fn insert(&mut self, device: DeviceDef) {
        self.0.insert(device.addr, device);
    }

    pub fn get(&self, addr: u8) -> Option<&DeviceDef> {
        self.0.get(&addr)
    }

    pub fn devices(&self) -> impl Iterator<Item = &DeviceDef> {
        self.0.values()
    }

    pub fn remove(&mut self, addr: u8) -> Option<DeviceDef> {
        self.0.remove(&addr)
    }

    pub fn len(&self) -> usize {
        self.0.len()
    }
}

impl<'a> IntoIterator for &'a DeviceDefList {
    type Item = &'a DeviceDef;
    type IntoIter = btree_map::Values<'a, u8, DeviceDef>;
    fn into_iter(self) -> Self::IntoIter {
        self.0.values()
    }
}
