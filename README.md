# Schwarzschild Lab

A real-time black hole you can play with in the browser. A GPU ray tracer bends every pixel's light ray around a Schwarzschild black hole, while a Rust physics engine compiled to WebAssembly moves planets, stars and gas clouds along exact relativistic orbits.

![Gravitationally lensed accretion disk](images/physics.png)

![Spacetime fabric view with orbiting bodies](images/spacetime.png)

## What's inside

**Ray tracer (WebGL2 / GLSL)**
- Null geodesics integrated per pixel with adaptive RK4: coarse far away, fine near the photon sphere
- Thin Keplerian accretion disk with a Novikov-Thorne temperature profile
- Relativistic Doppler shift, beaming and gravitational redshift from each photon's conserved angular momentum
- Blackbody colours from a Planck spectrum integrated against the CIE 1931 colour-matching functions
- HDR pipeline with a bloom chain, plus a procedural star field and galactic band

**Physics engine (Rust → WebAssembly)**
- Massive bodies follow the exact Schwarzschild timelike geodesic equation, so orbits precess, the ISCO sits at 6M and the photon sphere at 3M
- Run in a distant observer's time (bodies redden and freeze at the horizon) or in each body's proper time
- Optional body-to-body attraction, tidally shredded gas clouds and a hole that grows as it swallows mass

**Presets:** Gargantua, Physical (Doppler), Orbit zoo, Tidal disruption, Plunge & freeze, Spacetime fabric

## Run it

```sh
./build.sh
```

This builds the engine to `web/engine.wasm` and serves the app at <http://localhost:8765>. It needs Rust with the `wasm32-unknown-unknown` target:

```sh
rustup target add wasm32-unknown-unknown
```

A prebuilt `web/engine.wasm` is committed, so to try it without Rust you can serve the `web/` folder with any static server.

## Controls

| Input | Action |
| --- | --- |
| Drag | Orbit the camera |
| Wheel | Zoom |
| Click the plane | Spawn a planet, star, gas cloud or swarm |
| Click a body | Inspect it |
| `H` | Hide the panel |
| `Space` | Pause |

Units are geometric (G = c = 1) with every length measured in black-hole masses M; the event horizon is at r = 2M.
