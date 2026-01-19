// Landing page - minimal WASM initialization without Elm UI
import "./landing.css";

let flux;

// Hardcoded settings for the landing page background
const settings = {
  mode: "Normal",
  seed: null,
  fluidSize: 128,
  fluidFrameRate: 60.0,
  fluidTimestep: 1.0 / 60.0,
  viscosity: 5.0,
  velocityDissipation: 0.0,
  pressureMode: { ClearWith: 0.0 },
  diffusionIterations: 3,
  pressureIterations: 19,
  colorMode: { Preset: "Original" },
  lineLength: 450.0,
  lineWidth: 9.0,
  lineBeginOffset: 0.4,
  lineVariance: 0.55,
  gridSpacing: 15,
  viewScale: 1.6,
  noiseMultiplier: 0.45,
  noiseChannels: [
    { scale: 2.8, multiplier: 1.0, offsetIncrement: 0.001 },
    { scale: 15.0, multiplier: 0.7, offsetIncrement: 0.006 },
    { scale: 30.0, multiplier: 0.5, offsetIncrement: 0.012 },
  ],
};

async function initFlux() {
  const canvas = document.getElementById("canvas");

  try {
    // Check WebGPU support
    const hasWebGPU = navigator.gpu && await navigator.gpu.requestAdapter();

    if (hasWebGPU) {
      console.log("Backend: WebGPU");
      // Load WebGPU WASM module
      const wasm = await import(/* webpackIgnore: true */ "./flux/flux_wasm.js");
      await wasm.default("./flux/flux_wasm_bg.wasm");
      flux = await new wasm.Flux(settings);
    } else {
      console.log("Backend: WebGL2");
      // Load WebGL2 WASM module
      const wasm = await import(/* webpackIgnore: true */ "./flux-gl/flux_gl_wasm.js");
      await wasm.default("./flux-gl/flux_gl_wasm_bg.wasm");
      flux = new wasm.Flux(settings);
    }

    // Animation loop
    function animate(timestamp) {
      flux.animate(timestamp);
      window.requestAnimationFrame(animate);
    }

    // Handle canvas resize
    const resizeObserver = new ResizeObserver(([entry]) => {
      const { width, height } = entry.contentRect;
      if (width > 0 && height > 0) {
        flux.resize(width, height);
      }
    });
    resizeObserver.observe(canvas);

    // Start animation
    window.requestAnimationFrame(animate);

    // Remove loading class once initialized
    document.body.classList.remove("loading");
    console.log("Drift animation initialized successfully");
  } catch (error) {
    console.error("Failed to initialize Drift animation:", error);
    // Keep the gradient background as fallback
    document.body.classList.add("loading");
  }
}

// Initialize when DOM is ready
if (document.readyState === "loading") {
  document.addEventListener("DOMContentLoaded", initFlux);
} else {
  initFlux();
}
