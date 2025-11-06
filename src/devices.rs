use crate::device_list::{DeviceDef, DeviceDefList};
use crate::tag_ranges::TagRanges;
use crate::tags::{Tags, Updated as UpdatedTags};
use futures::future;
use std::sync::Arc;

#[derive(Clone)]
pub struct Device {
    unit: u8,
    tags: Tags,
    ranges: Arc<TagRanges>,
}

#[derive(Clone)]
pub struct Devices(Vec<Device>);

#[derive(Debug)]
pub enum Error {
    UnitNotAvailabe,
    LockFailed,
}

impl std::error::Error for Error {}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        use Error::*;
        match self {
            UnitNotAvailabe => write!(f, "Unit not available"),
            LockFailed => write!(f, "Unit not available"),
        }
    }
}
fn get_unit(dev: &Device) -> u8 {
    dev.unit
}

impl Devices {
    pub fn new(init: &DeviceDefList) -> Devices {
        let mut devs: Vec<Device> = Vec::new();
        for DeviceDef {
            tags: tag_list,
            addr,
        } in init
        {
            let tags = Tags::new(&tag_list);
            let ranges = Arc::new(TagRanges::from(&*tag_list));
            let dev = Device {
                unit: *addr,
                tags,
                ranges,
            };
            devs.push(dev);
        }
        devs.sort_by_key(get_unit);
        Devices(devs)
    }

    fn find_unit(&self, unit: u8) -> Option<&Device> {
        match self.0.binary_search_by_key(&unit, get_unit) {
            Ok(index) => Some(&self.0[index]),
            Err(_) => None,
        }
    }

    pub fn tags_read<F, R>(&self, unit: u8, f: F) -> Result<R, Error>
    where
        F: FnOnce(&Tags) -> R,
    {
        let Some(dev) = self.find_unit(unit) else {
            return Err(Error::UnitNotAvailabe);
        };
        let tags = &dev.tags;
        Ok(f(tags))
    }

    pub fn tags_write<F, R>(&self, unit: u8, f: F) -> Result<R, Error>
    where
        F: FnOnce(&Tags) -> R,
    {
        let Some(dev) = self.find_unit(unit) else {
            return Err(Error::UnitNotAvailabe);
        };
        let tags = &dev.tags;
        Ok(f(tags))
    }
    pub fn ranges(&self, unit: u8) -> Result<&TagRanges, Error> {
        let Some(dev) = self.find_unit(unit) else {
            return Err(Error::UnitNotAvailabe);
        };
        Ok(&dev.ranges)
    }

    pub async fn updated(&self) -> (u8, UpdatedTags) {
        let notify = future::select_all(self.0.iter().map(|dev| Box::pin(dev.tags.updated())));
        let (updated, index, _) = notify.await;
        let unit = self.0[index].unit;
        (unit, updated)
    }
    /// Iterate over unit numbers
    pub fn units(&self) -> impl Iterator<Item = u8> {
        self.0.iter().map(|d| d.unit)
    }
}
