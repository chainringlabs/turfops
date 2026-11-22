pub mod homeassistant;
pub mod openrouter;
pub mod openweathermap;
// The standalone source clients; used by the fetch schedule when it lands.
#[allow(dead_code)]
pub mod grid;
#[cfg(test)]
mod live_tests;
#[allow(dead_code)]
pub mod open_meteo;
#[allow(dead_code)]
pub mod outside;
#[allow(dead_code)]
pub mod scan;
#[allow(dead_code)]
pub mod uscrn;
pub mod weather;

pub use homeassistant::HomeAssistantClient;
pub use openrouter::OpenRouterClient;
pub use openweathermap::OpenWeatherMapClient;
pub use weather::WeatherLakeClient;
