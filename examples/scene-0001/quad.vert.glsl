#include "common.glsl"

out vec4 v_color;
void main() {
  mat4 Projection = projection();
  mat4 Camera = camera();
  gl_Position = Projection * Camera * vec4(quad(vertexId), 0, 1);
  v_color = vec4(1, 0, 0, 0.5);
}