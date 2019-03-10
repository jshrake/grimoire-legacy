// values from
// http://rastergrid.com/blog/2010/09/efficient-gaussian-blur-with-linear-sampling/
const float offsets[3] = float[](0.0, 1.3846153846, 3.2307692308);  // in pixels
const float weights[3] = float[](0.2270270270, 0.3162162162, 0.0702702703);

void mainImage(out vec4 fragColor, in vec2 fragCoord) {
  // Normalized pixel coordinates (from 0 to 1)
  vec2 uv = fragCoord / iResolution.xy;
  // Output to screen
  vec3 color = texture(iChannel0, uv).rgb * weights[0];
  for (int i = 1; i < 3; i++) {
    vec2 offset_uv = vec2(offsets[i] / iResolution.y, 0.0);
    float weight = weights[i];
    color += texture(iChannel0, uv + offset_uv).rgb * weight;
    color += texture(iChannel0, uv - offset_uv).rgb * weight;
  }
  fragColor.rgb = color;
  fragColor.a = 1.0;
}

out vec4 fragColor;
void main() { mainImage(fragColor, gl_FragCoord.xy); }
