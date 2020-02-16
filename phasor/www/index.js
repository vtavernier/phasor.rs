import * as wasm from "phasor";

let state = wasm.init(document.getElementById('canvas'));
window.requestAnimationFrame(function () {
  wasm.render(state);
});
