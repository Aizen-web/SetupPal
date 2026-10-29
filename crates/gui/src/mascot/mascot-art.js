/**
 * SetupPal: Authentic Kawaii Vector Art Assets
 * Matching the exact visual style, mascots, and UI illustrations
 */

/**
 * Mascot Face Icon (Sidebar Logo & Top Bar Pill)
 * @param {number} size
 */
export function createMascotHeadIconSvg(size = 28) {
  return `
<svg width="${size}" height="${size}" viewBox="0 0 40 40" fill="none" xmlns="http://www.w3.org/2000/svg">
  <!-- Ears -->
  <circle cx="11" cy="11" r="7" fill="#FFFFFF" stroke="#443933" stroke-width="2.5" />
  <circle cx="11" cy="11" r="3.5" fill="#F7D4C8" />
  <circle cx="29" cy="11" r="7" fill="#FFFFFF" stroke="#443933" stroke-width="2.5" />
  <circle cx="29" cy="11" r="3.5" fill="#F7D4C8" />
  
  <!-- Face Body -->
  <ellipse cx="20" cy="22" rx="16" ry="14" fill="#FFFFFF" stroke="#443933" stroke-width="2.5" />
  
  <!-- Blush -->
  <ellipse cx="10" cy="24" rx="3.5" ry="2" fill="#FFAAA6" opacity="0.8" />
  <ellipse cx="30" cy="24" rx="3.5" ry="2" fill="#FFAAA6" opacity="0.8" />
  
  <!-- Eyes (Happy / Sweet) -->
  <ellipse cx="14" cy="20" rx="2" ry="2.6" fill="#443933" />
  <circle cx="14.6" cy="19" r="0.8" fill="#FFFFFF" />
  <ellipse cx="26" cy="20" rx="2" ry="2.6" fill="#443933" />
  <circle cx="26.6" cy="19" r="0.8" fill="#FFFFFF" />
  
  <!-- Nose & Smile -->
  <circle cx="20" cy="23" r="1.2" fill="#443933" />
  <path d="M 18 25 Q 20 27 22 25" stroke="#443933" stroke-width="1.8" stroke-linecap="round" fill="none" />
</svg>
`;
}

/**
 * Byte (Cyber Cat) Face Icon
 * @param {number} size
 */
export function createByteIconSvg(size = 28) {
  return `
<svg width="${size}" height="${size}" viewBox="0 0 40 40" fill="none" xmlns="http://www.w3.org/2000/svg">
  <!-- Cat Ears -->
  <path d="M 6 18 L 12 6 L 19 14 Z" fill="#F4EDE4" stroke="#443933" stroke-width="2.2" stroke-linejoin="round" />
  <path d="M 8 16 L 12 9 L 16 14 Z" fill="#FFAAA6" opacity="0.8" />
  <path d="M 34 18 L 28 6 L 21 14 Z" fill="#F4EDE4" stroke="#443933" stroke-width="2.2" stroke-linejoin="round" />
  <path d="M 32 16 L 28 9 L 24 14 Z" fill="#FFAAA6" opacity="0.8" />
  
  <!-- Head -->
  <ellipse cx="20" cy="23" rx="15" ry="13" fill="#F4EDE4" stroke="#443933" stroke-width="2.4" />
  
  <!-- Cheeks -->
  <ellipse cx="11" cy="26" rx="3" ry="1.8" fill="#FFAAA6" opacity="0.75" />
  <ellipse cx="29" cy="26" rx="3" ry="1.8" fill="#FFAAA6" opacity="0.75" />
  
  <!-- Eyes -->
  <ellipse cx="15" cy="22" rx="2" ry="2.5" fill="#443933" />
  <circle cx="15.5" cy="21.2" r="0.8" fill="#FFFFFF" />
  <ellipse cx="25" cy="22" rx="2" ry="2.5" fill="#443933" />
  <circle cx="25.5" cy="21.2" r="0.8" fill="#FFFFFF" />
  
  <!-- Whiskers -->
  <line x1="7" y1="23" x2="2" y2="22" stroke="#443933" stroke-width="1.2" stroke-linecap="round" />
  <line x1="7" y1="26" x2="3" y2="27" stroke="#443933" stroke-width="1.2" stroke-linecap="round" />
  <line x1="33" y1="23" x2="38" y2="22" stroke="#443933" stroke-width="1.2" stroke-linecap="round" />
  <line x1="33" y1="26" x2="37" y2="27" stroke="#443933" stroke-width="1.2" stroke-linecap="round" />
  
  <!-- Nose & Cat Mouth -->
  <polygon points="19,24 21,24 20,25.5" fill="#443933" />
  <path d="M 18 26 Q 20 27.5 22 26" stroke="#443933" stroke-width="1.6" stroke-linecap="round" fill="none" />
</svg>
`;
}

/**
 * Mochi (Fluffy Bunny) Face Icon
 * @param {number} size
 */
export function createMochiIconSvg(size = 28) {
  return `
<svg width="${size}" height="${size}" viewBox="0 0 40 40" fill="none" xmlns="http://www.w3.org/2000/svg">
  <!-- Bunny Ears -->
  <path d="M 11 18 C 9 8, 12 3, 15 4 C 18 5, 17 12, 16 18 Z" fill="#FFFFFF" stroke="#443933" stroke-width="2.2" stroke-linejoin="round" />
  <path d="M 12 16 C 11 10, 13 6, 14.5 6.5 C 16 7, 15.5 12, 15 16 Z" fill="#F7D4C8" />
  <path d="M 29 18 C 31 8, 28 3, 25 4 C 22 5, 23 12, 24 18 Z" fill="#FFFFFF" stroke="#443933" stroke-width="2.2" stroke-linejoin="round" />
  <path d="M 28 16 C 29 10, 27 6, 25.5 6.5 C 24 7, 24.5 12, 25 16 Z" fill="#F7D4C8" />
  
  <!-- Head -->
  <ellipse cx="20" cy="24" rx="15" ry="13" fill="#FFFFFF" stroke="#443933" stroke-width="2.4" />
  
  <!-- Cheeks -->
  <ellipse cx="10" cy="26" rx="3.5" ry="2" fill="#FFAAA6" opacity="0.8" />
  <ellipse cx="30" cy="26" rx="3.5" ry="2" fill="#FFAAA6" opacity="0.8" />
  
  <!-- Eyes -->
  <ellipse cx="14.5" cy="22" rx="1.8" ry="2.4" fill="#443933" />
  <circle cx="15.2" cy="21.2" r="0.7" fill="#FFFFFF" />
  <ellipse cx="25.5" cy="22" rx="1.8" ry="2.4" fill="#443933" />
  <circle cx="26.2" cy="21.2" r="0.7" fill="#FFFFFF" />
  
  <!-- Nose & Bunny Mouth -->
  <ellipse cx="20" cy="24.5" rx="1.4" ry="1" fill="#FFAAA6" />
  <path d="M 18.5 26.5 Q 20 28 21.5 26.5" stroke="#443933" stroke-width="1.6" stroke-linecap="round" fill="none" />
</svg>
`;
}

/**
 * Top Hero Mascot Artwork:
 * Chubby white mascot lying flat on stomach typing on laptop,
 * with radiating vibe doodle lines on the left.
 */
export function createHeroLyingMascotSvg(width = 250, height = 135) {
  return `
<svg id="hero-lying-mascot" class="hero-lying-mascot-svg" width="${width}" height="${height}" viewBox="0 0 250 135" fill="none" xmlns="http://www.w3.org/2000/svg" role="img" aria-label="Setup Pal typing on laptop">
  <!-- Ground Shadows -->
  <g class="mascot-ground-shadow">
    <ellipse cx="125" cy="122" rx="72" ry="7" fill="rgba(68, 57, 51, 0.12)" />
    <ellipse cx="192" cy="123" rx="28" ry="5" fill="rgba(68, 57, 51, 0.16)" />
  </g>

  <!-- Radiating Doodle Lines on Left matching reference -->
  <g class="vibe-lines" stroke="#443933" stroke-width="2.6" stroke-linecap="round" fill="none">
    <path d="M 64 68 C 55 72, 53 79, 58 84" />
    <path d="M 50 82 C 40 87, 40 96, 48 102" />
    <path d="M 55 101 C 47 106, 48 113, 56 117" />
  </g>

  <!-- Laptop Base & Open Lid -->
  <g id="laptop-group" class="laptop-group">
    <!-- Screen / Open Lid tilted up -->
    <polygon points="174,106 218,106 211,66 181,66" fill="#3D332D" stroke="#332A24" stroke-width="2.4" stroke-linejoin="round" />
    
    <!-- White Logo Silhouette on back of lid (Bear / Apple silhouette) -->
    <g transform="translate(195, 82)">
      <circle cx="-3" cy="-4" r="2.2" fill="#FAF5EE" />
      <circle cx="3" cy="-4" r="2.2" fill="#FAF5EE" />
      <ellipse cx="0" cy="0" rx="4.8" ry="4.4" fill="#FAF5EE" />
    </g>

    <!-- Laptop Base / Keyboard -->
    <polygon points="158,120 220,120 215,106 170,106" fill="#4A3F38" stroke="#332A24" stroke-width="2.4" stroke-linejoin="round" />
    
    <!-- Subtle keyboard grid line -->
    <polygon points="172,116 212,116 208,109 176,109" fill="#332A24" opacity="0.6" />
  </g>

  <!-- Mascot Body Group (Lying down on tummy) -->
  <g id="mascot-body-lying" class="mascot-body-lying">
    <!-- Tail (cute round fluffy nub) -->
    <circle cx="68" cy="102" r="8" fill="#FFFFFF" stroke="#443933" stroke-width="2.6" />
    
    <!-- Left Hind Foot tucked in -->
    <ellipse cx="78" cy="116" rx="9" ry="6" fill="#FFFFFF" stroke="#443933" stroke-width="2.6" />
    <path d="M 72 116 Q 78 118 84 116" stroke="#443933" stroke-width="1.8" stroke-linecap="round" />

    <!-- Main Chubby Body (curves up smoothly to neck and head) -->
    <path d="M 75 114
             C 70 102, 74 86, 92 78
             C 106 72, 118 73, 128 77
             C 142 66, 168 70, 172 88
             C 176 102, 166 116, 146 119
             C 120 122, 92 122, 75 114 Z"
          fill="#FFFFFF"
          stroke="#443933"
          stroke-width="2.8"
          stroke-linejoin="round" />

    <!-- Left Ear -->
    <path d="M 124 72 C 120 60, 131 54, 136 64 Z" fill="#FFFFFF" stroke="#443933" stroke-width="2.6" stroke-linejoin="round" />
    <path d="M 126 70 C 124 63, 130 60, 133 65 Z" fill="#F8D3C8" />

    <!-- Right Ear -->
    <path d="M 152 74 C 150 62, 161 58, 164 68 Z" fill="#FFFFFF" stroke="#443933" stroke-width="2.6" stroke-linejoin="round" />
    <path d="M 154 72 C 153 65, 159 62, 161 67 Z" fill="#F8D3C8" />

    <!-- Head Outline Overlap -->
    <path d="M 126 77 C 132 68, 158 69, 166 82 C 172 91, 169 104, 158 108 C 146 112, 132 108, 126 98 C 122 91, 122 83, 126 77 Z"
          fill="#FFFFFF"
          stroke="#443933"
          stroke-width="2.8"
          stroke-linejoin="round" />

    <!-- Pink Cheeks -->
    <ellipse cx="128" cy="94" rx="4.5" ry="2.6" fill="#FFAAA6" opacity="0.8" />
    <ellipse cx="162" cy="95" rx="4.5" ry="2.6" fill="#FFAAA6" opacity="0.8" />

    <!-- Cute Eyes: Happy smiling arcs matching reference -->
    <path d="M 134 88 Q 138 84 142 88" stroke="#443933" stroke-width="2.4" stroke-linecap="round" fill="none" />
    <path d="M 150 88 Q 154 84 158 88" stroke="#443933" stroke-width="2.4" stroke-linecap="round" fill="none" />

    <!-- Little Nose & :3 Smile -->
    <circle cx="145" cy="92" r="1.4" fill="#443933" />
    <path d="M 141 94 Q 145 97 149 94" stroke="#443933" stroke-width="2" stroke-linecap="round" fill="none" />

    <!-- Paws Extended Forward Typing on Laptop -->
    <!-- Back paw (left) -->
    <g class="paw-left-typing">
      <ellipse cx="166" cy="108" rx="7" ry="5" fill="#FFFFFF" stroke="#443933" stroke-width="2.4" />
      <line x1="168" y1="106" x2="171" y2="109" stroke="#443933" stroke-width="1.4" stroke-linecap="round" />
    </g>

    <!-- Front paw (right) tapping laptop keys -->
    <g class="paw-right-typing">
      <ellipse cx="178" cy="112" rx="8" ry="5.5" fill="#FFFFFF" stroke="#443933" stroke-width="2.4" />
      <line x1="181" y1="110" x2="184" y2="114" stroke="#443933" stroke-width="1.4" stroke-linecap="round" />
    </g>
  </g>
</svg>
`;
}

/**
 * Tasks Panel: Clipboard Empty State Illustration
 */
export function createClipboardTaskArtSvg(size = 110) {
  return `
<svg width="${size}" height="${size}" viewBox="0 0 110 110" fill="none" xmlns="http://www.w3.org/2000/svg" role="img" aria-label="Tasks clipboard">
  <!-- Soft ground shadow -->
  <ellipse cx="55" cy="98" rx="36" ry="6" fill="rgba(68, 57, 51, 0.12)" />

  <!-- Clipboard Board (Warm tan wood) -->
  <rect x="25" y="20" width="60" height="74" rx="8" fill="var(--clipboard-board, #D7C6B0)" stroke="var(--clipboard-outline, #443933)" stroke-width="2.6" />

  <!-- White Paper Sheet -->
  <rect x="31" y="26" width="48" height="62" rx="4" fill="var(--clipboard-paper, #FAF6F0)" stroke="var(--clipboard-line, #E6DAC7)" stroke-width="1.8" />

  <!-- Metal Clip at Top -->
  <path d="M 42 16 C 42 12, 68 12, 68 16 L 68 25 L 42 25 Z" fill="var(--clipboard-clip, #695A4E)" stroke="var(--clipboard-outline, #443933)" stroke-width="2.2" stroke-linejoin="round" />
  <circle cx="55" cy="13" r="4" fill="none" stroke="var(--clipboard-outline, #443933)" stroke-width="2" />
  <circle cx="55" cy="21" r="2.2" fill="var(--clipboard-paper, #FAF6F0)" />

  <!-- Mini Terminal Screen on Paper -->
  <rect x="37" y="34" width="36" height="28" rx="4" fill="var(--clipboard-screen, #3D332D)" stroke="var(--clipboard-outline, #443933)" stroke-width="1.8" />
  <text x="42" y="52" font-family="'JetBrains Mono', Consolas, monospace" font-size="12" font-weight="700" fill="var(--clipboard-screen-text, #FAF5EE)">>_</text>

  <!-- Note lines underneath -->
  <line x1="38" y1="70" x2="72" y2="70" stroke="var(--clipboard-line, #D7C6B0)" stroke-width="2.2" stroke-linecap="round" />
  <line x1="38" y1="76" x2="62" y2="76" stroke="var(--clipboard-line, #D7C6B0)" stroke-width="2.2" stroke-linecap="round" />
</svg>
`;
}

/**
 * Tasks Panel Bottom: Sleeping Mascot on Stack of 3 Books with "z z z"
 */
export function createSleepingMascotOnBooksSvg(width = 170, height = 135) {
  return `
<svg width="${width}" height="${height}" viewBox="0 0 170 135" fill="none" xmlns="http://www.w3.org/2000/svg" role="img" aria-label="Sleeping mascot on books">
  <!-- Ground Shadow -->
  <ellipse cx="88" cy="126" rx="64" ry="7" fill="rgba(68, 57, 51, 0.14)" />

  <!-- Book 1 (Bottom book: Dark slate-blue / leather) -->
  <g class="book-bottom">
    <rect x="28" y="104" width="114" height="19" rx="3" fill="#44555A" stroke="#3D332D" stroke-width="2.4" />
    <!-- Cream Pages Edge -->
    <rect x="126" y="107" width="13" height="13" rx="1" fill="#FAF6F0" stroke="#DCD0BF" stroke-width="1.2" />
    <!-- Spine Ridges -->
    <line x1="36" y1="104" x2="36" y2="123" stroke="#3D332D" stroke-width="1.8" />
    <line x1="42" y1="104" x2="42" y2="123" stroke="#3D332D" stroke-width="1.8" />
  </g>

  <!-- Book 2 (Middle book: Warm Terracotta / Caramel) -->
  <g class="book-middle">
    <rect x="34" y="87" width="108" height="18" rx="3" fill="#B56E45" stroke="#3D332D" stroke-width="2.4" />
    <!-- Cream Pages Edge -->
    <rect x="128" y="90" width="11" height="12" rx="1" fill="#FAF6F0" stroke="#DCD0BF" stroke-width="1.2" />
    <!-- Spine line -->
    <line x1="42" y1="87" x2="42" y2="105" stroke="#3D332D" stroke-width="1.8" />
  </g>

  <!-- Book 3 (Top book: Dark Espresso with gold accent) -->
  <g class="book-top">
    <rect x="42" y="70" width="98" height="18" rx="3" fill="#3D332D" stroke="#332A24" stroke-width="2.4" />
    <!-- Cream Pages Edge -->
    <rect x="124" y="73" width="13" height="12" rx="1" fill="#FAF6F0" stroke="#DCD0BF" stroke-width="1.2" />
    <!-- Gold Ribbon Bookmark dangling -->
    <path d="M 68 88 L 68 102 L 72 98 L 76 102 L 76 88 Z" fill="#E8B548" stroke="#3D332D" stroke-width="1.2" />
  </g>

  <!-- Chubby Mascot Fast Asleep on Top of Books -->
  <g class="sleeping-mascot-group">
    <!-- Fluffy Round Tail -->
    <circle cx="116" cy="62" r="7" fill="#FFFFFF" stroke="#443933" stroke-width="2.4" />

    <!-- Body (Curled up bean shape resting on book surface) -->
    <path d="M 58 72
             C 52 62, 58 48, 74 44
             C 90 40, 114 44, 118 58
             C 122 70, 114 74, 98 74
             C 80 74, 64 74, 58 72 Z"
          fill="#FFFFFF"
          stroke="#443933"
          stroke-width="2.6"
          stroke-linejoin="round" />

    <!-- Ears -->
    <path d="M 58 46 C 54 38, 62 34, 66 42 Z" fill="#FFFFFF" stroke="#443933" stroke-width="2.4" stroke-linejoin="round" />
    <path d="M 60 44 C 57 39, 62 37, 64 42 Z" fill="#FFAAA6" />
    
    <path d="M 76 46 C 74 38, 82 34, 85 43 Z" fill="#FFFFFF" stroke="#443933" stroke-width="2.4" stroke-linejoin="round" />
    <path d="M 77 44 C 76 39, 81 37, 83 42 Z" fill="#FFAAA6" />

    <!-- Head Curve -->
    <ellipse cx="64" cy="58" rx="14" ry="12" fill="#FFFFFF" stroke="#443933" stroke-width="2.4" />

    <!-- Pink Cheek Blush -->
    <ellipse cx="54" cy="62" rx="3.5" ry="2" fill="#FFAAA6" opacity="0.8" />
    <ellipse cx="74" cy="62" rx="3.5" ry="2" fill="#FFAAA6" opacity="0.8" />

    <!-- Closed Happy Sleeping Eyes: ⌒ ⌒ -->
    <path d="M 56 57 Q 59 53 62 57" stroke="#443933" stroke-width="2" stroke-linecap="round" fill="none" />
    <path d="M 66 57 Q 69 53 72 57" stroke="#443933" stroke-width="2" stroke-linecap="round" fill="none" />

    <!-- Cute Nose & Mouth -->
    <circle cx="64" cy="61" r="1.2" fill="#443933" />
    <path d="M 62 63 Q 64 65 66 63" stroke="#443933" stroke-width="1.4" stroke-linecap="round" fill="none" />

    <!-- Paws Tucked In Under Chin -->
    <ellipse cx="60" cy="71" rx="5" ry="3.5" fill="#FFFFFF" stroke="#443933" stroke-width="2" />
    <ellipse cx="70" cy="71" rx="5" ry="3.5" fill="#FFFFFF" stroke="#443933" stroke-width="2" />
  </g>

  <!-- Floating Sleeping Zzzs -->
  <g class="sleeping-zzz-group">
    <text x="110" y="44" font-family="'Pixelify Sans', monospace" font-size="12" font-weight="700" fill="#75665B" class="zzz-anim-1">z</text>
    <text x="120" y="32" font-family="'Pixelify Sans', monospace" font-size="15" font-weight="700" fill="#75665B" class="zzz-anim-2">z</text>
    <text x="132" y="19" font-family="'Pixelify Sans', monospace" font-size="18" font-weight="700" fill="#75665B" class="zzz-anim-3">Z</text>
  </g>
</svg>
`;
}
