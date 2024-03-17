#![cfg_attr(not(feature = "export-abi"), no_std)]
#![cfg_attr(not(any(feature = "export-abi", test)), no_main)]
extern crate alloc;

/// Use an efficient WASM allocator.
#[global_allocator]
static ALLOC: mini_alloc::MiniAlloc = mini_alloc::MiniAlloc::INIT;

use alloc::{string::String, vec::Vec};
use alloy_primitives::Signed;
use alloy_sol_types::sol;
use stylus_sdk::{
    alloy_primitives::{Address, U256},
    evm, msg,
    prelude::*,
    storage::{StorageAddress, StorageI128, StorageMap, StorageString, StorageU256, StorageVec},
};
use substrate_fixed::types::I64F64;
use substrate_geohash::GeoHash;

#[entrypoint]
#[solidity_storage]
pub struct Geocab {
    number: StorageU256,
    drivers_on_grid: StorageMap<String, StorageVec<DriverLocation>>,
    driver_grid: StorageMap<Address, StorageString>,
    active_trips: StorageVec<Trip>,
}

#[solidity_storage]
pub struct DriverLocation {
    pub address: StorageAddress,
    pub lat: StorageI128,
    pub lon: StorageI128,
}

impl DriverLocation {
    fn location(&self) -> Location {
        let lat = to_fixed_signed(self.lat.get());
        let lon = to_fixed_signed(self.lon.get());
        Location { lat, lon }
    }
}

#[solidity_storage]
pub struct Trip {
    pub passenger: StorageAddress,
    pub driver: StorageAddress,
}

/// Declare that `Geocab` is a contract with the following external methods.
#[external]
impl Geocab {
    /// Publish driver locations
    pub fn publish_driver_locations(&mut self, drivers: Vec<(Address, i128, i128)>) {
        //self.number.set(drivers[0].1);
        for driver in drivers {
            let (address, lat_input, lon_input) = driver;
            let hash = encode_geohash(lat_input, lon_input);
            // mapping driver address -> geohash
            let mut guard = self.driver_grid.setter(address);
            guard.set_str(&hash);
            // mapping geohash -> driver
            let mut guard = self.drivers_on_grid.setter(hash);
            let mut driver = guard.grow();
            driver.address.set(address);
            driver.lat.set(lat_input.try_into().unwrap());
            driver.lon.set(lon_input.try_into().unwrap());
        }
    }

    /// Gets drivers at a geohash
    pub fn drivers_at_geohash(&self, geohash: String) -> Result<Vec<Address>, Vec<u8>> {
        let mut result = Vec::new();
        let drivers = self.drivers_on_grid.get(geohash);
        for i in 0..drivers.len() {
            result.push(drivers.get(i).unwrap().address.get())
        }
        Ok(result)
    }

    /// Books a trip
    pub fn book_trip(&mut self, origin: (i128, i128), destination: (i128, i128)) {
        let origin_hash = encode_geohash(origin.0, origin.1);
        let passenger_location = Location::from_i128_tuple(origin);
        let nearby_drivers = self.drivers_on_grid.get(origin_hash);
        let driver_locations = get_locations(&nearby_drivers);
        let closest_driver_index = closest_index(&driver_locations, passenger_location);
        let driver_location = nearby_drivers
            .get(closest_driver_index)
            .expect("No drivers");
        let mut new_trip = self.active_trips.grow();
        new_trip.driver.set(driver_location.address.get());
        new_trip.passenger.set(msg::sender());
        evm::log(TripBooked {
            passenger: msg::sender(),
            driver: driver_location.address.get(),
            dest_lat: destination.0,
            dest_lon: destination.1,
        });
    }

    pub fn active_passengers(&self) -> Result<Vec<Address>, Vec<u8>> {
        let mut result = Vec::new();
        for i in 0..self.active_trips.len() {
            result.push(self.active_trips.get(i).unwrap().passenger.get())
        }
        Ok(result)
    }

    /// Gets the number from storage.
    pub fn number(&self) -> Result<U256, Vec<u8>> {
        Ok(U256::from(self.number.get()))
    }

    /// Sets a number in storage to a user-specified value.
    pub fn set_number(&mut self, new_number: U256) {
        self.number.set(new_number);
    }

    /// Increments `number` and updates its value in storage.
    pub fn increment(&mut self) {
        let number = self.number.get();
        self.set_number(number + U256::from(1));
    }
}

fn get_locations(drivers: &StorageVec<DriverLocation>) -> Vec<Location> {
    let mut result = Vec::new();
    for i in 0..drivers.len() {
        result.push(drivers.get(i).unwrap().location())
    }
    result
}

fn closest_index(locations: &[Location], passanger: Location) -> usize {
    let mut min = I64F64::max_value();
    let mut result = 0;
    for (index, location) in locations.iter().enumerate() {
        let distance = location.distance_indication(&passanger);
        if distance < min {
            min = distance;
            result = index
        }
    }
    return result;
}
struct Location {
    pub lat: I64F64,
    pub lon: I64F64,
}

impl Location {
    pub fn from_i128_tuple(t: (i128, i128)) -> Self {
        Location {
            lat: to_fixed(t.0),
            lon: to_fixed(t.1),
        }
    }

    pub fn distance_indication(&self, other: &Location) -> I64F64 {
        let lat_diff = self.lat - other.lat;
        let lon_diff = self.lon - other.lon;
        lat_diff.abs() + lon_diff.abs()
    }
}

#[inline]
fn to_fixed(x: i128) -> I64F64 {
    I64F64::from_be_bytes(x.to_be_bytes())
}

#[inline]
fn to_fixed_signed(x: Signed<128, 2>) -> I64F64 {
    I64F64::from_be_bytes(x.to_be_bytes())
}

fn encode_geohash(lat: i128, lon: i128) -> String {
    let lat = to_fixed(lat);
    let lon = to_fixed(lon);
    GeoHash::<9>::try_from_params(lat, lon).unwrap().into()
}

fn all_neighbors(center: &GeoHash<5>) -> Vec<GeoHash<5>> {
    let mut result = Vec::new();
    let neighbors = center.neighbors().expect("Invalid hash");
    result.push(neighbors.n);
    result.push(neighbors.ne);
    result.push(neighbors.e);
    result.push(neighbors.se);
    result.push(neighbors.s);
    result.push(neighbors.sw);
    result.push(neighbors.w);
    result.push(neighbors.nw);
    result
}

pub enum GeocabError {
    InvalidGeohashLength(InvalidGeohashLength),
    GenericError(GenericError),
}

sol! {
    event TripBooked(address indexed passenger, address indexed driver, int128 dest_lat, int128 dest_lon);

    error InvalidTokenId(uint256 token_id);
    error NotOwner(address from, uint256 token_id, address real_owner);
    error NotApproved(uint256 token_id, address owner, address spender);
    error TransferToZero(uint256 token_id);
    error ReceiverRefused(address receiver, uint256 token_id, bytes4 returned);
    error InvalidGeohashLength();
    error GenericError();
}

#[cfg(test)]
mod tests {
    use core::str::FromStr;

    use alloc::{string::String, vec::Vec};
    use substrate_fixed::types::I64F64;
    use substrate_geohash::GeoHash;

    use crate::encode_geohash;

    #[test]
    fn test_geohash() {
        let hash = encode_geohash(940783947759187132416, 0);
        assert_eq!("gcpfpurbx", hash)
    }

    #[test]
    fn test_geo_2() {
        let lat = I64F64::from_str("51.0").unwrap();
        let lon = I64F64::from_str("0.0").unwrap();
        let geohash = GeoHash::<5>::try_from_params(lat, lon).unwrap();
        let neighbors = geohash.neighbors().unwrap();
        let geo_str = geohash.into();
        let mut results = Vec::<String>::new();
        results.push(geo_str);
        results.push(neighbors.n.into());
        results.push(neighbors.ne.into());
        results.push(neighbors.e.into());
        results.push(neighbors.se.into());
        results.push(neighbors.s.into());
        results.push(neighbors.sw.into());
        results.push(neighbors.w.into());
        results.push(neighbors.nw.into());
        assert_eq!(
            "gcpfp, gcpfr, u1042, u1040, u101b, gcpcz, gcpcy, gcpfn, gcpfq",
            results.join(", ")
        );
    }

    #[test]
    fn test_num() {
        let n = 51_i128 << 64;
        assert_eq!(940783947759187132416, n)
    }
}
