void mainImage(out vec4 fragColor, in vec2 fragCoord) {
  // Normalized pixel coordinates (from 0 to 1)
  vec2 uv = (iResolution.xy - fragCoord) / iResolution.xy;
  // Output to screen
  fragColor = texture(iChannel0, uv);
}

out vec4 fragColor;
void main() { mainImage(fragColor, gl_FragCoord.xy); }
