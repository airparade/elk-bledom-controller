use elk_bledom_controller::device::{BledomDevice, BledomError};
use log::{error, info};
use tokio::time::{self, Duration};

#[tokio::main]
async fn main() -> Result<(), BledomError> {
    env_logger::init();

    info!("starting Bledom device example...");

    let device = match BledomDevice::builder().build().await {
        Ok(dev) => dev,
        Err(e) => {
            error!("failed to initialize BledomDevice: {}", e);
            return Err(e);
        }
    };

    info!("bledomDevice initialized successfully.");

    info!("turning on the light...");
    if let Err(e) = device.power_on().await {
        error!("failed to power on the device: {}", e);
        return Err(e);
    }
    info!("light is ON.");
    time::sleep(Duration::from_secs(2)).await;

    info!("setting brightness to 50%...");
    if let Err(e) = device.set_brightness(50).await {
        error!("failed to set brightness: {}", e);
        return Err(e);
    }
    info!("brightness set to 50%.");
    time::sleep(Duration::from_secs(2)).await;

    info!("setting brightness to 100%...");
    if let Err(e) = device.set_brightness(100).await {
        error!("failed to set brightness: {}", e);
        return Err(e);
    }
    info!("brightness set to 100%.");
    time::sleep(Duration::from_secs(2)).await;

    info!("setting color to RED...");
    if let Err(e) = device.set_color(255, 0, 0).await {
        error!("failed to set color to red: {}", e);
        return Err(e);
    }
    info!("color set to RED.");
    time::sleep(Duration::from_secs(2)).await;

    info!("setting color to GREEN...");
    if let Err(e) = device.set_color(0, 255, 0).await {
        error!("failed to set color to green: {}", e);
        return Err(e);
    }
    info!("color set to GREEN.");
    time::sleep(Duration::from_secs(2)).await;

    info!("setting color to BLUE...");
    if let Err(e) = device.set_color(0, 0, 255).await {
        error!("failed to set color to blue: {}", e);
        return Err(e);
    }

    info!("turning off the light...");
    if let Err(e) = device.power_off().await {
        error!("failed to power off the device: {}", e);
        return Err(e);
    }
    info!("light is OFF.");
    time::sleep(Duration::from_secs(1)).await;

    info!("bledom device example finished.");
    Ok(())
}
