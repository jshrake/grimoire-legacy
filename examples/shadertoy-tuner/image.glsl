void mainImage( out vec4 O,  vec2 U )
{

    vec2 R = iResolution.xy;
    U /= R;
    float scale = U.y < 1./4. ? 1. : U.y < 2./4. ? 4. :  U.y < 3./4. ?  8. : 16.;
    U.y = mod(U.y,1./4.)*4.;

    //float M = iSampleRate;
    float M = 48000.;

    // last FFT value in texture = iSampleRate/4
    #define freq(f) abs( (f)/(M/4.)  -U.x) * R.x * scale

    U.x /= scale;
    O = texture(iChannel0,vec2(U.x,.25));

    if (U.y<0. || U.y>.5) return;

    if (freq(165.)<.5) O.g++;    // E0 guitar
    if (freq(220.)<.5) O.g++;    // A0
    if (freq(294.)<.5) O.g++;    // D1
    if (freq(392.)<.5) O.g++;    // G1
    if (freq(494.)<.5) O.g++;    // B1
    if (freq(660.)<.5) O.g++;    // E2
    if (freq( 588.)<.5) O.b++;   //  D2 flute
    if (freq(784.)<.5) O.b++;    //  G2
    if (freq(1046.)<.5) O.b++;   //  C3
 // if (freq(1150.)<.5) O.b++;   //
    if (freq(1175.)<.5) O.b++;   //  D3
    if (U.y<.25) {
        if (freq( 220.)<1.) O++;
        if (freq( 440.)<2.) O++; // A1
        if (freq( 880.)<1.) O++; // A2
        if (freq(1760.)<.5) O++; // A3
        if (freq(3520.)<.5) O++; // A4
        if (freq(7040.)<.5) O++; // A5
    }
}

out vec4 fragColor;
void main() { mainImage(fragColor, gl_FragCoord.xy); }
