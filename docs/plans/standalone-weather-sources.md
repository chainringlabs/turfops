---
title: Weather and soil data for a standalone, US-wide TurfOps
project: turfops
status: research
created: 2026-11-22
updated: 2026-11-22
issue:
summary: Survey and accuracy tests of free weather and soil data sources, with a recommended tiered design that lets TurfOps run anywhere in the lower 48 without the Dagster weather lake.
---

# Research: weather and soil data for a standalone, US-wide TurfOps

**Status: research and recommendation. Nothing here is designed or approved.** Three decisions
are open (see "Open decisions"); the HLD revision starts once they are answered.

## Recommendation in brief

1. **Estimate everywhere from free gridded air temperature.** PRISM daily max/min, fetched
   keyless from RCC-ACIS, dates freezes and growing degree days within about a week at 80 % of
   station-years. Soil temperature is modelled from that air series with the app's existing
   air→soil regression, calibrated on the five nearest USCRN stations.
2. **Use a measured station when one is near enough for that input.** USCRN first, USDA SCAN
   second. For soil that is 50 miles (about 47 % of the US population); for hourly air and
   humidity about 10; for daily air and rain PRISM at the lawn's point is better than a station
   at any tested distance beyond 5 miles.
3. **Let the owner's own sensors override both.** Home Assistant already supplies patio air
   temperature and humidity; a soil probe would slot in the same way.
4. **Withhold what cannot be estimated honestly.** Soil moisture has no free estimate worth
   showing; without a station or probe it is absent, not guessed.
5. **Say which basis every answer rests on**: measured at a named station and distance,
   estimated for the location, or the owner's sensor.

No source in the plan needs a paid plan or an API key. The one hard dependency this removes is
the Dagster-built parquet lake.

## Why this is needed

TurfOps reads one NOAA USCRN station (PA Avondale) from a parquet lake built by a separate
Dagster project. That is the only hard blocker to running the app elsewhere: Home Assistant,
OpenWeatherMap and OpenRouter are already optional. Two things have to change for anyone else
to use it:

- **The pipeline.** Someone installing TurfOps cannot be asked to run Dagster first.
- **The station.** USCRN has 121 stations with working soil probes in the whole country.

## What the app consumes

| Input | Used for | Today |
|---|---|---|
| Daily air max/min/mean, 15 years | Freeze dates, GDD, soil model training, typical dates | Station, via lake |
| Daily soil temperature at 5 cm and 10 cm, 15 years | Timing windows, seasonal plan, grub window | Station, via lake |
| Hourly air temperature and humidity, last 14 days | Disease models (humid hours, night lows) | Station, via lake |
| Rainfall, hourly and 7-day | Irrigation, rain delay, watering reminders | Station, via lake |
| Soil moisture | Irrigation, fertilizer block | Station, via lake |
| Forecast, 5+ days | Outlooks, application window, disease outlook | OpenWeatherMap (keyed) |
| Ambient temperature and humidity now | Dashboard, fertilizer block | Home Assistant |

## Sources surveyed

All checks were live requests made 2026-11-21 unless marked otherwise.

### Measured soil temperature

| Source | Access | Coverage | Notes |
|---|---|---|---|
| NOAA USCRN | Keyless files | 121 stations with soil | Hourly at 5 depths, daily product, 5-minute product (5 cm only). NOAA's own lag was about 43 hours when checked. Probes fail: three Midwest stations lost theirs in 2025 |
| USDA SCAN (AWDB REST) | Keyless JSON | 207 stations | Depths 2/4/8/20/40 in, about 1 hour behind, history from about 2000. Soil temperature in whole °F; multi-month sensor outages; query per state |
| USDA SNOTEL | Same API | 482 with soil | Mountain sites, little use for lawns |
| State mesonets | Mixed | Patchy | Keyless with soil: IA, MT, CO, AZ, ND/MN, TX, WI, FL (live only). Free key: CA, NC, IL. Nothing keyless in the Northeast, Mid-Atlantic or Southeast |

There is no free national aggregator of measured soil temperature. Synoptic and MADIS are
restricted; GHCN-Daily carries soil at only 157 current US stations.

### Gridded and modelled

| Source | Access | Resolution | Current through | Notes |
|---|---|---|---|---|
| PRISM via RCC-ACIS GridData (grid 21) | Keyless JSON POST | About 4 km, daily | Yesterday | Max/min air, precipitation, GDD; from 1981. PRISM terms allow free reuse with attribution. ACIS publishes no terms or rate limits |
| NRCC grids via ACIS (grids 1 and 3) | Same | About 4 km, daily | Today | Grid 1 from 1950, CONUS. Not scored yet |
| Open-Meteo archive (ERA5, ERA5-Land, IFS) | Keyless | 9–25 km | About 5 days | Includes 0–7 cm soil temperature. Free tier is non-commercial, CC BY 4.0, 10,000 calls a day |
| Open-Meteo forecast and historical forecast | Keyless | 2.5–13 km | Live | Hourly air, humidity, rain, soil; 16-day forecast; 93 past days |
| NASA POWER | Keyless | About 50 km | 3 days behind | Too coarse |
| RTMA / URMA | Keyless S3 | 2.5 km hourly | Live | GRIB2 only; the only gridded hourly dew point found |
| Daymet, gridMET, NLDAS, SMAP | Mixed | — | — | No current year, or login required |

### Station air, humidity and rain

| Source | Access | Notes |
|---|---|---|
| IEM ASOS archive | Keyless, public domain | Hourly temperature, humidity, rain at about 2,700 US airports; decades of history; about 50 minutes behind; one request per second |
| NWS API observations | Keyless, User-Agent required | Only 7 days of history |
| RCC-ACIS StnData | Keyless | Daily max/min/rain at airport and co-op stations, 1 day behind |
| NCEI normals 1991–2020 | Keyless | Freeze-date probabilities for about 15,200 stations |

NCEI's classic hourly archive stopped updating in 2025. Meteostat mixes forecast rows into its
station files. No free source gives measured hourly humidity at an arbitrary point.

### Forecast

| Source | Access | Notes |
|---|---|---|
| Open-Meteo | Keyless | Hourly, 16 days, includes humidity, rain, soil. Non-commercial free tier |
| NWS API | Keyless, "free for any purpose" | Hourly to about 6.5 days, rain amounts about 3 days, US only |
| OpenWeatherMap | Free key | Current provider; 5 days, 3-hourly |

## Coverage of measured soil stations

Share of US population within a given distance of a station with working soil probes
(county-centroid estimate, 2020 census):

| Network | 25 mi | 50 mi | 75 mi | 100 mi |
|---|---|---|---|---|
| USCRN | 6.3 % | 27.4 % | 60.0 % | 80.1 % |
| SCAN | 8.5 % | 27.3 % | 42.6 % | 62.3 % |
| USCRN + SCAN | 13.7 % | 47.3 % | 78.0 % | 92.1 % |

The median person is 52 miles from the nearest one. Coverage is uneven: Michigan has 5 % within
50 miles, New Jersey 11 %, Indiana 11 %, while Missouri has 95 %. Detroit's nearest station is
128 miles away, Pittsburgh's 103.

## Accuracy tests

**Method.** Ground truth is NOAA's USCRN daily product at 133 lower-48 stations, 2012–2025
(113 have five or more usable soil years). Each candidate source was asked to reproduce the
dates TurfOps actually computes:

- 5 cm soil, 5-day mean, held 5 days: rising through 50 °F and 55 °F, falling through 70 °F and 55 °F
- first fall freeze and last spring freeze (air minimum ≤ 32 °F)
- the day cumulative GDD50 reaches 200

Results are **median | 80th-percentile absolute error in days**. Bias is the mean signed
error; positive means later than measured.

**Scale.** One station differs from its own typical date by 4–8 days in a median year and
11–16 days at the 80th percentile. An error smaller than that is within ordinary year-to-year
variation.

### How far can a station's dates be borrowed?

Copying a neighbouring station's soil crossing dates, by distance between the pair
(pairs differing by more than 1,000 ft in elevation excluded):

| Distance | Spring 55 °F |
|---|---|
| 0–30 mi | 2 \| 8 |
| 60–100 mi | 5 \| 17 |
| 100–150 mi | 8 \| 22 |
| 200–300 mi | 12 \| 28 |

Borrowing is sound to roughly 60 miles and no better than the calendar past 100. Even
co-located pairs differ by 8–17 days at the 80th percentile, which is the noise floor of the
measurement itself.

### Air-derived dates from PRISM (all stations, 2012–2025)

| Metric | Error | Bias |
|---|---|---|
| First fall freeze | 1 \| 9 | +3.5 |
| Last spring freeze | 1 \| 8 | −3.0 |
| GDD reaches 200 | 2 \| 5 | +1.6 |

### A neighbouring station's air against PRISM at the point

Copying a neighbouring station's air-derived dates, against PRISM at the target station, by
distance between the pair (station copy vs PRISM; elevation difference at most 1,000 ft):

| Distance | Pairs | First freeze | Last freeze | GDD reaches 200 | Daily min error, °F |
|---|---|---|---|---|---|
| 0–5 mi | 4 | 0 \| 6 vs 1 \| 8 | 0 \| 7 vs 1 \| 5 | 1 \| 5 vs 2 \| 5 | 1.5 vs 3.0 |
| 5–15 mi | 3 | 1 \| 18 vs 1 \| 9 | 2 \| 15 vs 1 \| 7 | 2 \| 5 vs 2 \| 4 | 3.0 vs 3.0 |
| 15–30 mi | 10 | 1 \| 10 vs 1 \| 9 | 1 \| 19 vs 1 \| 9 | 1 \| 4 vs 1 \| 4 | 2.1 vs 3.1 |
| 30–45 mi | 5 | 0 \| 6 vs 1 \| 7 | 0 \| 19 vs 1 \| 4 | 2 \| 6 vs 2 \| 6 | 1.6 vs 2.7 |
| 45–60 mi | 16 | 1 \| 13 vs 1 \| 7 | 0 \| 18 vs 1 \| 6 | 3 \| 8 vs 2 \| 5 | 2.4 vs 2.9 |
| 60–100 mi | 47 | 1 \| 14 vs 1 \| 7 | 1 \| 23 vs 1 \| 6 | 6 \| 13 vs 2 \| 5 | 3.1 vs 2.9 |

Typical years agree; the station's bad years are far worse for freeze dates beyond 5 miles.
PRISM's daily error is inflated by its 12Z–12Z day, which does not hurt dates. Only 38 pairs
sit within 60 miles, so single bins are rough; the direction is the same in every bin.

### Soil crossings from PRISM air plus a regression trained elsewhere

No soil measurement from the target station is used.

| Calibrated on | Spring 50 | Spring 55 | Fall 70 | Fall 55 |
|---|---|---|---|---|
| The station itself (best possible) | 1 \| 8 | 2 \| 12 | 2 \| 8 | 3 \| 10 |
| Nearest station | 2 \| 14 | 3 \| 16 | 3 \| 17 | 4 \| 12 |
| **Nearest five, pooled** | **2 \| 12** | **3 \| 14** | **3 \| 15** | **4 \| 12** |
| All other stations, pooled | 2 \| 14 | 3 \| 17 | 4 \| 19 | 6 \| 14 |

The fall 55 °F crossing runs about 6 days late under the nearest-five calibration. Unlike
copying dates, this method does not degrade with distance to the calibration station: where the
nearest station is 60–120 miles away it scores 5 | 19 on spring 55 °F, against 16 | 35 for
copying.

### Open-Meteo reanalysis, same 16 stations, 2016–2025

| Source | Spring 55 | Fall 70 | First freeze | Last freeze |
|---|---|---|---|---|
| Open-Meteo archive, default | 4 \| 21, bias −4.2 | 3 \| 21 | 8 \| 30, bias +14.8 | 3 \| 22, bias −10.7 |
| Open-Meteo ERA5-Land | 4 \| 18, bias −4.6 | 2 \| 20 | 16 \| 35, bias +18.2 | 11 \| 26, bias −14.6 |
| PRISM air + nearest-five regression | 2 \| 12, bias −0.3 | 3 \| 18 | — | — |
| PRISM air | — | — | 1 \| 11, bias +2.4 | 1 \| 7, bias −3.2 |

The reanalysis warms soil too early in spring and misses frosts: it puts the first fall freeze
two to three weeks late on average. The finer ERA5-Land grid does not fix either. This rules
Open-Meteo out as the basis for freeze dates or soil crossings.

### Typical (multi-year median) dates

The timing windows depend on a station's typical date as much as on this year's.

| Source | Result |
|---|---|
| PRISM air | First freeze 4 \| 8, last freeze 3 \| 8, GDD 200 2 \| 5 |
| Nearest-five regression | Soil crossings 4–5 \| 8–11 |
| Copy nearest station | Soil crossings 8–10 \| 14–28 |
| Open-Meteo default | Soil crossings 6–10 \| 10–18; first freeze 12 \| 18 |

### Humid hours for the disease models

Hours per day at ≥ 90 % humidity, May–September 2025, eight stations, against measured USCRN
hourly data:

| Source | Bias (h/day) | Mean abs diff (h) | Agreement on "≥ 10 humid hours" days | Night-low bias |
|---|---|---|---|---|
| Open-Meteo archive | −0.8 | 3.4 | 80 % | +1.8 °F |
| Open-Meteo historical forecast | −1.3 | 2.9 | 84 % | +1.2 °F |
| Nearest airport (4–11 mi) | +0.6 | 3.1 | 83 % | +0.3 °F |
| Second-nearest airport | +0.2 | 4.0 | 76 % | +1.6 °F |

Every remote source is wrong by about three hours a day. Airports miss in both directions: at
Avondale the station measured 9.0 humid hours a day while the two nearest airports gave 13.4
and 4.1. Disease risk from remote data has to be labelled approximate.

### 10 cm soil and the warmer thresholds

The grub window and seasonal plan use 10 cm soil with a 7-day mean. Scored the same way
(nearest-five calibration, 113 stations), with a day-of-year term added to the regression:

| 10 cm threshold | Estimate | Year-to-year spread |
|---|---|---|
| Rising 60 °F | 4 \| 16 | 6 \| 13 |
| Rising 65 °F | 5 \| 17 | 6 \| 13 |
| Rising 75 °F | 9 \| 21 | 6 \| 14 |
| Falling 65 °F | 3 \| 13 | 5 \| 11 |
| Falling 45 °F | 4 \| 12 | 6 \| 11 |

- Depth makes no difference: 5 cm scores the same on these thresholds, and the daily fit is
  slightly tighter at 10 cm (RMSE 3.2 °F against 3.5 °F).
- The day-of-year term is required. Without it every warming threshold runs 3–8 days late,
  because soil runs warmer than air under summer sun.
- The rising 75 °F crossing is not usable from the estimate: it is worse than the location's
  typical date.

### Every crossing the app dates, on one footing

PRISM air regression with a day-of-year sine and cosine, 113 stations at 5 cm and 109 at 10 cm,
2012–2025. 5 cm on the timing rule (5-day mean held 5 days), 10 cm on the 7-day mean held
5 days. "One-sided" counts station-years where the measured soil crossed and the estimate did
not, then the reverse.

| Crossing | Nearest five | Bias | One-sided | Self-trained | Between years |
|---|---|---|---|---|---|
| 5 cm rising 45 °F | 2 \| 12 | −2.7 | 0 / 0 | 1 \| 7 | 5 \| 14 |
| 5 cm rising 50 °F | 2 \| 12 | −1.9 | 0 / 0 | 1 \| 7 | 5 \| 12 |
| 5 cm rising 55 °F | 2 \| 13 | −1.6 | 0 / 3 | 1 \| 8 | 6 \| 13 |
| 5 cm rising 65 °F | 5 \| 17 | +0.4 | 0 / 15 | 2 \| 10 | 6 \| 14 |
| 5 cm falling 75 °F | 1 \| 11 | −1.4 | 0 / 0 | 0 \| 6 | 2 \| 9 |
| 5 cm falling 70 °F | 2 \| 15 | 0.0 | 1 / 3 | 1 \| 8 | 4 \| 12 |
| 5 cm falling 65 °F | 3 \| 13 | +2.3 | 3 / 2 | 1 \| 7 | 5 \| 11 |
| 5 cm falling 55 °F | 3 \| 10 | +3.5 | 11 / 27 | 2 \| 7 | 6 \| 11 |
| 5 cm falling 40 °F (from Nov 1) | 4 \| 12 | +5.1 | 45 / 30 | 3 \| 11 | 4 \| 10 |
| 10 cm rising 50 °F | 3 \| 12 | −2.0 | 0 / 0 | 1 \| 6 | 5 \| 13 |
| 10 cm rising 55 °F | 3 \| 13 | −1.7 | 1 / 3 | 2 \| 7 | 6 \| 13 |
| 10 cm rising 60 °F | 4 \| 15 | −1.4 | 0 / 1 | 2 \| 9 | 6 \| 13 |
| 10 cm rising 65 °F | 5 \| 17 | +0.7 | 0 / 17 | 2 \| 10 | 6 \| 13 |
| 10 cm rising 75 °F | 8 \| 20 | +4.5 | 150 / 53 | 3 \| 13 | 6 \| 14 |
| 10 cm falling 65 °F | 3 \| 14 | +1.9 | 2 / 5 | 1 \| 6 | 5 \| 11 |
| 10 cm falling 55 °F | 3 \| 10 | +3.1 | 23 / 18 | 2 \| 6 | 5 \| 10 |
| 10 cm falling 50 °F | 3 \| 11 | +3.6 | 19 / 23 | 2 \| 8 | 6 \| 10 |
| 10 cm falling 45 °F | 4 \| 12 | +5.3 | 29 / 29 | 3 \| 10 | 6 \| 11 |

Daily error (RMSE): 3.5 °F at 5 cm and 3.1 °F at 10 cm from the nearest five; 2.7 °F and
2.3 °F self-trained. From the nearest five, every crossing but 10 cm rising 75 °F and 5 cm falling 40 °F beats the
between-year spread at the median; self-trained, all do.

These are the research scripts' figures. The official figures are the scoring tool's
(`backend/src/logic/soil_estimate/scores.json`), which dates each crossing with the consuming
component's own code. They agree to within a day except where a consumer's scan range differs
from the script's: scanned through January 31 as the dormant-seeding window does, 5 cm
falling 40 °F scores 3.5 | 12 against a between-year spread of 5 | 12, and is datable.

### Are PRISM's recent days as good as its settled history?

Grid minus station, daily values as served on 2026-11-21, against the same calendar days of
2025 as a control (mean absolute error, °F):

| Age of the day | PRISM min, 2026 / control | PRISM max, 2026 / control |
|---|---|---|
| 0–3 days | 4.1 / 3.5 | 5.7 / 4.4 |
| 4–7 days | 2.9 / 4.0 | 4.2 / 4.7 |
| 8–30 days | 3.0 / 3.7 | 4.6 / 5.5 |
| 31–90 days | 2.6 / 2.5 | 4.4 / 4.2 |

From four days old, recent PRISM values are no worse than settled ones. The newest three days
are somewhat worse (a small sample of 127 station-days). First fall freeze in 2026, dated
entirely from recent values, scored 1 | 6 at 93 stations, the same as 2025.

The NRCC interpolated grid, which reports through the current day, is weaker on freeze dates:
4 | 19 in 2026 and 2 | 11 in 2025, with a freeze seen on one side and not the other at 8–9
stations. Over 2012–2025 it scored first freeze 1 | 17, last freeze 1 | 20 and GDD 200 3 | 9,
against PRISM's 1 | 9, 1 | 8 and 2 | 5. PRISM stays the basis; NRCC is a fallback, still
better than reanalysis.

### Rain

Daily precipitation at the station's point against the station's gauge, 2024–2025, 133
stations (mean weekly gauge total 17.2 mm):

| Source | 7-day total: abs error, bias (mm) | Weeks ≥ 25 mm: caught, false alarms | Days ≥ 2.5 mm: caught, false alarms |
|---|---|---|---|
| PRISM | 4.5, +0.4 | 88 %, 15 % | 82 %, 22 % |
| Open-Meteo archive | 8.2, −0.1 | 75 %, 27 % | 81 %, 33 % |
| Open-Meteo historical forecast | 10.8, −2.4 | 58 %, 22 % | 60 %, 16 % |

PRISM's daily value matches the gauge best when shifted one day (its day ends at 12Z). The
radar-gauge grid was not scored: the grid id tried was rejected by RCC-ACIS.

### Known weak spots

- Individual stations go badly wrong under every method: Millbrook NY (fall 70 °F about 22
  days early), Des Moines IA (spring 16 days early), Necedah WI (fall 24 days early).
- The mountain and arid West is thinly tested.
- The regression tested is a simplified stand-in, not the app's `fit_model`.

## Recommended design

### Tiers by data need

| Need | Everywhere (estimate) | Station | Owner's sensor |
|---|---|---|---|
| Daily air, freeze dates, GDD | PRISM via ACIS | Not used | Air sensor |
| Soil temperature 5 cm / 10 cm | Regression on PRISM air, calibrated on nearest five USCRN stations | USCRN, then SCAN, within 50 mi | Soil probe |
| 15-year typical dates | Same sources, 15 years back | Station record, for soil | — |
| Hourly air and humidity | Open-Meteo, labelled approximate | Station hourly, within 10 mi | Patio sensor |
| Rainfall | PRISM daily; Open-Meteo for the newest day | Not used | Rain gauge |
| Soil moisture | Withheld | Within 50 mi | Soil probe |
| Forecast | Open-Meteo by default; NWS or OpenWeatherMap selectable | — | — |

### Principles

- **The owner gives a location, nothing else.** The app picks the nearest stations and decides
  the tier per input; the owner can pin a station or turn one off.
- **Tiers are per input, not per install.** A lawn 40 miles from a station with a failed soil
  probe uses that station's air and the estimate for soil.
- **A station outage falls back to the estimate**, with the basis shown, instead of the
  calendar. This would also have covered Avondale's 2026 probe dropout.
- **Every answer carries its basis.** "Measured at Avondale, 12 mi" and "Estimated for your
  location" are different claims and the UI and MCP tools must say which.
- **Withhold, don't invent** stays: soil moisture and anything the estimate cannot support
  are absent.

### What this changes in the high-level design

| Today | Would become |
|---|---|
| Tenet: measured over modelled | Measured where available; a labelled estimate otherwise |
| Non-goal: modelled weather as a substitute for the station | Removed |
| Non-goal: one lawn, one station, one owner | One lawn and owner; location-driven sources |
| Decision: read the weather lake in place with DuckDB | The app fetches and stores its own weather data |
| Goal: no external calls when idle | Dropped; sources are fetched on a schedule |

### Implementation consequences to design, not yet decided

- The app fetches its own weather data on a schedule and stores it in Postgres (decided
  2026-11-22). Reading an external parquet lake is not a supported mode.
- How an owner maps their own sensors to inputs, and how those readings are stored.
- A 15-year backfill on first start: about 15 station-year files per USCRN station, or one ACIS
  request per grid point.
- What ambient-dependent rules do when Home Assistant is not configured has not been traced.

## Not yet established

1. **PRISM's newest three days** are somewhat less accurate than older ones; whether they are
   later revised was not measured.
2. **PRISM may be flattered** if it ingests the USCRN stations it was scored against; which
   networks it uses was not checked.
3. **Soil moisture is unvalidated**, a station's rain gauge by distance was not scored, and
   the 10-mile cut-off for hourly air and humidity rests on 8 stations.
4. **The app's real soil model was not the one tested.**
5. **ACIS has no published terms or rate limits**, and no equivalent keyless fallback was
   found for daily air: Open-Meteo is the only other candidate and is weak on freeze dates.
6. **Open-Meteo's free tier is non-commercial.** Fine for a self-hosted personal install;
   a constraint if the project ever hosts for others.
7. **SCAN was surveyed, not scored** against USCRN.

## Open decisions

1. **Is a labelled estimate acceptable as the base tier?** Decided 2026-11-22: yes. Past the
   cut-off each input is shown as an estimate for the location; inside it, as measured at the
   named station with its distance.
2. **Station cut-off.** Decided 2026-11-22: per input. Soil 50 miles (the data supports about
   60), hourly air and humidity 10, daily air and rain from PRISM at every distance.
3. **Default forecast provider.** Open-Meteo suggested, with NWS and OpenWeatherMap selectable.

## Provenance

The scoring scripts and downloaded data lived in a temporary session directory and are not in
the repository. The method section is sufficient to rebuild them: USCRN `daily01` files as
truth, ACIS GridData grid 21 for PRISM, the Open-Meteo archive API for reanalysis, IEM
`asos.py` for airports, and a least-squares fit of daily 5 cm soil on trailing air means over
1, 3, 7, 14 and 30 days.
