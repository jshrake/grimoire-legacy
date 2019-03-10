in vec4 v_color;
in vec2 v_uv;
out vec4 fragColor;

void main() {
  // ground plane w/ checkerboard pattern
  vec2 pos = floor(v_uv * 2000.0);
  float pattern = mod(pos.x + mod(pos.y, 2.0), 2.0);
  fragColor = pattern * vec4(1.);
  fragColor.a = 1.;
}