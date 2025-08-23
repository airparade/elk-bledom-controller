use btleplug::api::bleuuid::uuid_from_u16;
use btleplug::api::Characteristic;
use btleplug::api::{Central, Manager as _, Peripheral as _, ScanFilter, WriteType};
use btleplug::platform::{Adapter, Manager, Peripheral};
use log::{debug, error, info, warn};
use std::time::Duration;
use thiserror::Error;
use tokio::time;
use uuid::Uuid;

const LIGHT_CHARACTERISTIC_UUID: Uuid = uuid_from_u16(0xFFF3);
const CMD_DELAY: Duration = Duration::from_millis(100);

#[derive(Debug, Error)]
pub enum BledomError {
    #[error("Bluetooth manager error: {0}")]
    BluetoothManagerError(#[from] btleplug::Error),
    #[error("No Bluetooth adapters found")]
    NoAdaptersFound,
    #[error("Failed to start BLE scan: {0}")]
    ScanError(String),
    #[error("Could not find device after multiple tries")]
    DeviceNotFound,
    #[error("Failed to connect to device after multiple tries: {0}")]
    ConnectionFailed(String),
    #[error("Failed to discover services: {0}")]
    ServiceDiscoveryError(String),
    #[error("Light characteristic (UUID: {LIGHT_CHARACTERISTIC_UUID}) not found on device")]
    CharacteristicNotFound,
    #[error("Invalid parameter: {0}")]
    InvalidParameter(String),
    #[error("Other error: {0}")]
    Other(#[from] Box<dyn std::error::Error>),
}

#[derive(Debug)]
pub struct BledomDevice {
    peripheral: Peripheral,
    characteristic: Characteristic,
}

pub struct Days {
    pub monday: u8,
    pub tuesday: u8,
    pub wednesday: u8,
    pub thursday: u8,
    pub friday: u8,
    pub saturday: u8,
    pub sunday: u8,
    pub all: u8,
    pub week_days: u8,
    pub weekend_days: u8,
    pub none: u8,
}

pub const WEEK_DAYS: Days = Days {
    monday: 0x01,
    tuesday: 0x02,
    wednesday: 0x04,
    thursday: 0x08,
    friday: 0x10,
    saturday: 0x20,
    sunday: 0x40,
    all: 0x01 + 0x02 + 0x04 + 0x08 + 0x10 + 0x20 + 0x40,
    week_days: 0x01 + 0x02 + 0x04 + 0x08 + 0x10,
    weekend_days: 0x20 + 0x40,
    none: 0x00,
};

pub struct Effects {
    pub jump_red_green_blue: u8,
    pub jump_red_green_blue_yellow_cyan_magenta_white: u8,
    pub crossfade_red: u8,
    pub crossfade_green: u8,
    pub crossfade_blue: u8,
    pub crossfade_yellow: u8,
    pub crossfade_cyan: u8,
    pub crossfade_magenta: u8,
    pub crossfade_white: u8,
    pub crossfade_red_green: u8,
    pub crossfade_red_blue: u8,
    pub crossfade_green_blue: u8,
    pub crossfade_red_green_blue: u8,
    pub crossfade_red_green_blue_yellow_cyan_magenta_white: u8,
    pub blink_red: u8,
    pub blink_green: u8,
    pub blink_blue: u8,
    pub blink_yellow: u8,
    pub blink_cyan: u8,
    pub blink_magenta: u8,
    pub blink_white: u8,
    pub blink_red_green_blue_yellow_cyan_magenta_white: u8,
}

pub const EFFECTS: Effects = Effects {
    jump_red_green_blue: 0x87,
    jump_red_green_blue_yellow_cyan_magenta_white: 0x88,
    crossfade_red: 0x8b,
    crossfade_green: 0x8c,
    crossfade_blue: 0x8d,
    crossfade_yellow: 0x8e,
    crossfade_cyan: 0x8f,
    crossfade_magenta: 0x90,
    crossfade_white: 0x91,
    crossfade_red_green: 0x92,
    crossfade_red_blue: 0x93,
    crossfade_green_blue: 0x94,
    crossfade_red_green_blue: 0x89,
    crossfade_red_green_blue_yellow_cyan_magenta_white: 0x8a,
    blink_red: 0x96,
    blink_green: 0x97,
    blink_blue: 0x98,
    blink_yellow: 0x99,
    blink_cyan: 0x9a,
    blink_magenta: 0x9b,
    blink_white: 0x9c,
    blink_red_green_blue_yellow_cyan_magenta_white: 0x95,
};

#[derive(Default)]
pub struct BledomDeviceBuilder {
    scan_retries: Option<u8>,
    scan_interval_ms: Option<u64>,
    connection_retries: Option<u8>,
    connection_interval_ms: Option<u64>,
}

impl BledomDeviceBuilder {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn scan_retries(mut self, retries: u8) -> Self {
        self.scan_retries = Some(retries);
        self
    }

    pub fn scan_interval_ms(mut self, interval: u64) -> Self {
        self.scan_interval_ms = Some(interval);
        self
    }

    pub fn connection_retries(mut self, retries: u8) -> Self {
        self.connection_retries = Some(retries);
        self
    }

    pub fn connection_interval_ms(mut self, interval: u64) -> Self {
        self.connection_interval_ms = Some(interval);
        self
    }

    pub async fn build(self) -> Result<BledomDevice, BledomError> {
        let scan_retries = self.scan_retries.unwrap_or(10);
        let scan_interval_ms = self.scan_interval_ms.unwrap_or(1000);
        let connection_retries = self.connection_retries.unwrap_or(10);
        let connection_interval_ms = self.connection_interval_ms.unwrap_or(100);

        debug!("newing device...");
        let manager = Manager::new().await?;
        let central = get_central(&manager).await?;

        debug!("adapter in used:\n{:#?}", central);
        let mut light = None;

        central
            .start_scan(ScanFilter::default())
            .await
            .map_err(|e| BledomError::ScanError(e.to_string()))?;

        let mut find_count = 0;
        while light.is_none() {
            info!("trying to find light...");
            if find_count >= scan_retries {
                central.stop_scan().await.ok(); // Attempt to stop scan on error
                return Err(BledomError::DeviceNotFound);
            }
            match find_light(&central).await {
                Ok(p) => {
                    light = Some(p);
                }
                Err(BledomError::DeviceNotFound) => {}
                Err(e) => {
                    central.stop_scan().await.ok();
                    return Err(e);
                }
            }
            find_count += 1;
            time::sleep(Duration::from_millis(scan_interval_ms)).await;
        }

        central
            .stop_scan()
            .await
            .map_err(|e| BledomError::ScanError(format!("failed to stop scan: {}", e)))?;

        let lc = light.clone().ok_or(BledomError::DeviceNotFound)?;
        let mut connect_count = 0;
        let mut connect_status = false;
        while !connect_status {
            info!("trying to connect to light");
            match lc.connect().await {
                Ok(_) => {
                    connect_status = true;
                }
                Err(e) => {
                    warn!("failed to connect light: {}", e);
                    connect_count += 1;
                    if connect_count >= connection_retries {
                        return Err(BledomError::ConnectionFailed(e.to_string()));
                    } else {
                        time::sleep(Duration::from_millis(connection_interval_ms)).await;
                    }
                }
            }
        }

        lc.discover_services()
            .await
            .map_err(|e| BledomError::ServiceDiscoveryError(e.to_string()))?;

        let chars = lc.characteristics();

        let cmd_char = chars
            .iter()
            .find(|c| c.uuid == LIGHT_CHARACTERISTIC_UUID)
            .ok_or(BledomError::CharacteristicNotFound)?;

        let peripheral = light.unwrap();

        let device = BledomDevice {
            peripheral,
            characteristic: cmd_char.to_owned(),
        };
        Ok(device)
    }
}

impl BledomDevice {
    pub fn builder() -> BledomDeviceBuilder {
        BledomDeviceBuilder::new()
    }

    async fn send_command_bytes(&self, data: &[u8]) -> Result<(), BledomError> {
        if data.len() != 9 || data[0] != 0x7e || data[8] != 0xef {
            return Err(BledomError::InvalidParameter("malformed command byte array (expected 9 bytes, starting with 0x7e and ending with 0xef)".to_string()));
        }
        self.peripheral
            .write(&self.characteristic, data, WriteType::WithoutResponse)
            .await?;
        time::sleep(CMD_DELAY).await;
        Ok(())
    }

    pub async fn power_on(&self) -> Result<(), BledomError> {
        self.send_command_bytes(&[0x7e, 0x00, 0x04, 0xf0, 0x00, 0x01, 0xff, 0x00, 0xef])
            .await
    }

    pub async fn power_off(&self) -> Result<(), BledomError> {
        self.send_command_bytes(&[0x7e, 0x00, 0x04, 0x00, 0x00, 0x00, 0xff, 0x00, 0xef])
            .await
    }

    pub async fn set_brightness(&self, value: u8) -> Result<(), BledomError> {
        if value > 0x64 {
            return Err(BledomError::InvalidParameter(format!(
                "brightness value {value} out of supported range (0-100)."
            )));
        }
        self.send_command_bytes(&[0x7e, 0x00, 0x01, value, 0x00, 0x00, 0x00, 0x00, 0xef])
            .await
    }

    pub async fn sync_time(&self) -> Result<(), BledomError> {
        let system_time = chrono::offset::Local::now();
        let hour = chrono::Timelike::hour(&system_time) as u8;
        let minute = chrono::Timelike::minute(&system_time) as u8;
        let second = chrono::Timelike::second(&system_time) as u8;
        let day_of_week = chrono::Datelike::weekday(&system_time).number_from_monday() as u8; // 1 for Monday, 7 for Sunday
        self.send_command_bytes(&[
            0x7e,
            0x00,
            0x83,
            hour,
            minute,
            second,
            day_of_week,
            0x00,
            0xef,
        ])
        .await
    }

    pub async fn set_custom_time(
        &self,
        hour: u8,
        minute: u8,
        second: u8,
        day_of_week: u8, // 1 for Monday, 7 for Sunday
    ) -> Result<(), BledomError> {
        if hour > 23 {
            return Err(BledomError::InvalidParameter(format!(
                "hour value {hour} out of supported range (0-23)."
            )));
        }
        if minute > 59 {
            return Err(BledomError::InvalidParameter(format!(
                "minute value {minute} out of supported range (0-59)."
            )));
        }
        if second > 59 {
            return Err(BledomError::InvalidParameter(format!(
                "second value {second} out of supported range (0-59)."
            )));
        }
        if !(1..=7).contains(&day_of_week) {
            return Err(BledomError::InvalidParameter(format!(
                "day of week value {day_of_week} out of supported range (1-7, 1=Monday)."
            )));
        }

        self.send_command_bytes(&[
            0x7e,
            0x00,
            0x83,
            hour,
            minute,
            second,
            day_of_week,
            0x00,
            0xef,
        ])
        .await
    }

    pub async fn set_color(
        &self,
        red_value: u8,
        green_value: u8,
        blue_value: u8,
    ) -> Result<(), BledomError> {
        self.send_command_bytes(&[
            0x7e,
            0x00,
            0x05,
            0x03,
            red_value,
            green_value,
            blue_value,
            0x00,
            0xef,
        ])
        .await
    }

    pub async fn set_effect(&self, value: u8) -> Result<(), BledomError> {
        self.send_command_bytes(&[0x7e, 0x00, 0x03, value, 0x03, 0x00, 0x00, 0x00, 0xef])
            .await
    }

    pub async fn set_effect_speed(&self, value: u8) -> Result<(), BledomError> {
        if value > 0x64 {
            return Err(BledomError::InvalidParameter(format!(
                "effect speed value {value} out of supported range (0-100)."
            )));
        }
        self.send_command_bytes(&[0x7e, 0x00, 0x02, value, 0x00, 0x00, 0x00, 0x00, 0xef])
            .await
    }

    pub async fn set_schedule_on(
        &self,
        days: u8,
        hours: u8,
        minutes: u8,
        enabled: bool,
    ) -> Result<(), BledomError> {
        if days > WEEK_DAYS.all {
            return Err(BledomError::InvalidParameter(format!(
                "days bitmask {days:#02x} is invalid (max 0x7F)."
            )));
        }
        if hours > 23 {
            return Err(BledomError::InvalidParameter(format!(
                "hour value {hours} out of supported range (0-23)."
            )));
        }
        if minutes > 59 {
            return Err(BledomError::InvalidParameter(format!(
                "minute value {minutes} out of supported range (0-59)."
            )));
        }

        let value = if enabled { days + 0x80 } else { days };
        self.send_command_bytes(&[0x7e, 0x00, 0x82, hours, minutes, 0x00, 0x00, value, 0xef])
            .await
    }

    pub async fn set_schedule_off(
        &self,
        days: u8,
        hours: u8,
        minutes: u8,
        enabled: bool,
    ) -> Result<(), BledomError> {
        // Days are bit flags, valid range 0x00-0x7F (all bits 0-6 set for Monday-Sunday)
        if days > WEEK_DAYS.all {
            return Err(BledomError::InvalidParameter(format!(
                "days bitmask {days:#02x} is invalid (max 0x7F)."
            )));
        }
        if hours > 23 {
            return Err(BledomError::InvalidParameter(format!(
                "hour value {hours} out of supported range (0-23)."
            )));
        }
        if minutes > 59 {
            return Err(BledomError::InvalidParameter(format!(
                "minute value {minutes} out of supported range (0-59)."
            )));
        }

        let value = if enabled { days + 0x80 } else { days };
        self.send_command_bytes(&[0x7e, 0x00, 0x82, hours, minutes, 0x00, 0x01, value, 0xef])
            .await
    }

    pub async fn generic_command(
        &self,
        id: u8,
        sub_id: u8,
        arg1: u8,
        arg2: u8,
        arg3: u8,
    ) -> Result<(), BledomError> {
        self.send_command_bytes(&[0x7e, 0x00, id, sub_id, arg1, arg2, arg3, 0x00, 0xef])
            .await
    }
}

async fn get_central(manager: &Manager) -> Result<Adapter, BledomError> {
    debug!("getting adapters...");
    let adapters = manager.adapters().await?;

    debug!("adapters:\n{:#?}", adapters);
    if adapters.is_empty() {
        error!("no adapters found");
        Err(BledomError::NoAdaptersFound)
    } else {
        Ok(adapters.into_iter().next().unwrap())
    }
}

pub async fn find_light(central: &Adapter) -> Result<Peripheral, BledomError> {
    for p in central.peripherals().await? {
        if p.properties()
            .await?
            .ok_or(BledomError::Other(
                "Peripheral properties not available".into(),
            ))?
            .local_name
            .iter()
            .any(|name| name.contains("ELK-BLEDOM"))
        {
            return Ok(p);
        }
    }
    Err(BledomError::DeviceNotFound)
}
