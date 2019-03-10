void mainImage(out vec4 fragColor, in vec2 fragCoord) {
  // Normalized pixel coordinates (from 0 to 1)
  vec2 uv = fragCoord / iResolution.xy;
  // Output to screen
  if (fragCoord.x < iMouse.x) {
    fragColor.rgb = texture(iChannel0, uv).rgb;
  } else {
    fragColor.rgb = texture(iChannel1, uv).rgb;
  }
  fragColor.a = 0.0;
}

out vec4 fragColor;
void main() { mainImage(fragColor, gl_FragCoord.xy); }
