/**
 * SetupPal: Pixel the ghost, the only mascot.
 * Group IDs follow docs/UI.md 11.4 so ghost.css can target each part.
 */

/**
 * @param {number} size      rendered width and height in px
 * @param {string} idPrefix  keeps element IDs unique when several Pixels are on screen
 * @param {object} [opts]
 * @param {boolean} [opts.live]  mark as the mascot that store.setMascotState drives
 */
export function createPixelSvg(size = 140, idPrefix = '', { live = false } = {}) {
  const p = idPrefix ? `${idPrefix}-` : '';
  return `
<svg id="${p}ghost-root" class="ghost-root pet-root pet-ghost" ${live ? 'data-mascot-live="true"' : ''} viewBox="-12 -12 224 224" width="${size}" height="${size}" overflow="visible" xmlns="http://www.w3.org/2000/svg" role="img" aria-label="Pixel the ghost">
  <defs>
    <filter id="${p}neon-glow" x="-20%" y="-20%" width="140%" height="140%">
      <feGaussianBlur stdDeviation="3" result="blur" />
      <feMerge>
        <feMergeNode in="blur" />
        <feMergeNode in="SourceGraphic" />
      </feMerge>
    </filter>
    <linearGradient id="${p}ghost-grad" x1="0%" y1="0%" x2="0%" y2="100%">
      <stop offset="0%" stop-color="#F4EFFF" />
      <stop offset="100%" stop-color="#E2D7FF" />
    </linearGradient>
    <linearGradient id="${p}beam-grad" x1="0%" y1="0%" x2="100%" y2="50%">
      <stop offset="0%" stop-color="rgba(57, 255, 20, 0.55)" />
      <stop offset="100%" stop-color="rgba(57, 255, 20, 0)" />
    </linearGradient>
    <linearGradient id="${p}shield-grad" x1="0%" y1="0%" x2="100%" y2="100%">
      <stop offset="0%" stop-color="rgba(255, 75, 75, 0.65)" />
      <stop offset="100%" stop-color="rgba(255, 40, 40, 0.2)" />
    </linearGradient>
  </defs>

  <!-- #shadow -->
  <g id="${p}shadow" class="ghost-shadow">
    <ellipse cx="100" cy="180" rx="42" ry="9" fill="rgba(59, 47, 42, 0.16)" />
  </g>

  <!-- #ghost-body -->
  <g id="${p}ghost-body" class="ghost-body-group pet-body-group">
    <!-- Body Shape -->
    <g id="${p}body">
      <path d="M 60 152 
               C 56 120, 56 68, 100 68 
               C 144 68, 144 120, 140 152
               C 133 147, 127 147, 120 152
               C 113 157, 107 157, 100 152
               C 93 147, 87 147, 80 152
               C 73 157, 67 157, 60 152 Z" 
            fill="url(#${p}ghost-grad)" 
            stroke="#BBAEFF" 
            stroke-width="3.5" 
            stroke-linejoin="round" />
      <!-- Cute antennae -->
      <path d="M 72 74 Q 68 58 80 66 Z" fill="#E1D7FF" stroke="#BBAEFF" stroke-width="2.5" />
      <path d="M 128 74 Q 132 58 120 66 Z" fill="#E1D7FF" stroke="#BBAEFF" stroke-width="2.5" />
    </g>

    <!-- #hem -->
    <g id="${p}hem" class="ghost-hem">
      <path d="M 64 148 Q 72 156 80 148 Q 88 156 96 148 Q 104 156 112 148 Q 120 156 128 148 Q 136 156 140 148" 
            fill="none" 
            stroke="#D0C5FF" 
            stroke-width="2" 
            stroke-linecap="round" />
    </g>

    <!-- #face -->
    <g id="${p}face" class="ghost-face">
      <!-- Cheeks -->
      <circle id="${p}cheek-l" class="cheek-blush" cx="77" cy="114" r="6" fill="#F8BBD9" opacity="0.8" />
      <circle id="${p}cheek-r" class="cheek-blush" cx="123" cy="114" r="6" fill="#F8BBD9" opacity="0.8" />

      <!-- Eyes Normal -->
      <g id="${p}eyes-normal" class="eyes-normal">
        <g id="${p}eye-l" class="ghost-eye">
          <ellipse cx="82" cy="104" rx="5.5" ry="7" fill="#1E1B2E" />
          <circle cx="84" cy="102" r="2.2" fill="#FFFFFF" />
          <circle cx="81" cy="106" r="1.1" fill="#FFFFFF" />
        </g>
        <g id="${p}eye-r" class="ghost-eye">
          <ellipse cx="118" cy="104" rx="5.5" ry="7" fill="#1E1B2E" />
          <circle cx="120" cy="102" r="2.2" fill="#FFFFFF" />
          <circle cx="117" cy="106" r="1.1" fill="#FFFFFF" />
        </g>
      </g>

      <!-- Eyes Happy -->
      <g id="${p}eyes-happy" class="eyes-happy" opacity="0">
        <path d="M 76 106 Q 82 98 88 106" fill="none" stroke="#1E1B2E" stroke-width="3" stroke-linecap="round" />
        <path d="M 112 106 Q 118 98 124 106" fill="none" stroke="#1E1B2E" stroke-width="3" stroke-linecap="round" />
      </g>

      <!-- Eyes Sad / Crying -->
      <g id="${p}eyes-sad" class="eyes-sad" opacity="0">
        <!-- Teary glistening anime eyes -->
        <ellipse cx="82" cy="105" rx="5.5" ry="6.5" fill="#1E1B2E" />
        <circle cx="84" cy="103" r="2.5" fill="#64D2FF" />
        <ellipse cx="118" cy="105" rx="5.5" ry="6.5" fill="#1E1B2E" />
        <circle cx="120" cy="103" r="2.5" fill="#64D2FF" />
      </g>

      <!-- Brows -->
      <g id="${p}brows" class="ghost-brows" opacity="0">
        <path id="${p}brow-l" d="M 76 96 Q 82 93 88 97" fill="none" stroke="#7B61D6" stroke-width="2" stroke-linecap="round" />
        <path id="${p}brow-r" d="M 112 97 Q 118 93 124 96" fill="none" stroke="#7B61D6" stroke-width="2" stroke-linecap="round" />
      </g>

      <!-- Brows Sad -->
      <g id="${p}brows-sad" class="brows-sad" opacity="0">
        <path d="M 76 94 Q 82 98 88 95" fill="none" stroke="#7B61D6" stroke-width="2.5" stroke-linecap="round" />
        <path d="M 112 95 Q 118 98 124 94" fill="none" stroke="#7B61D6" stroke-width="2.5" stroke-linecap="round" />
      </g>

      <!-- Mouth -->
      <g id="${p}mouth" class="ghost-mouth">
        <path class="mouth-smile" d="M 96 112 Q 100 117 104 112" fill="none" stroke="#7B61D6" stroke-width="2.5" stroke-linecap="round" />
        <ellipse class="mouth-open" cx="100" cy="115" rx="3.5" ry="4.5" fill="#7B61D6" opacity="0" />
        <path class="mouth-wobble" d="M 94 115 Q 97 111 100 115 Q 103 119 106 115" fill="none" stroke="#7B61D6" stroke-width="2.5" stroke-linecap="round" opacity="0" />
        <!-- Sad pout / frown for errors and red state -->
        <path class="mouth-sad" d="M 95 117 Q 100 111 105 117" fill="none" stroke="#7B61D6" stroke-width="2.5" stroke-linecap="round" opacity="0" />
      </g>
    </g>

    <!-- Arms -->
    <g id="${p}arms">
      <path id="${p}arm-l" class="pet-arm-l" d="M 61 116 Q 52 120 57 127 Q 63 127 65 120" fill="#E1D7FF" stroke="#BBAEFF" stroke-width="2.5" />
      <path id="${p}arm-r" class="pet-arm-r" d="M 139 116 Q 148 120 143 127 Q 137 127 135 120" fill="#E1D7FF" stroke="#BBAEFF" stroke-width="2.5" />
      <path id="${p}acc-hand" class="acc-hand" d="M 139 118 Q 148 111 153 117 Q 157 123 149 128 L 140 126 Z" fill="#E1D7FF" stroke="#BBAEFF" stroke-width="2.5" opacity="0" />
    </g>

    <!-- #acc-hardhat -->
    <g id="${p}acc-hardhat" class="acc-group" opacity="0">
      <path d="M 75 72 C 75 52, 125 52, 125 72 Z" fill="#FFB800" stroke="#CC8800" stroke-width="2.5" />
      <rect x="70" y="70" width="60" height="7" rx="3.5" fill="#FFA500" stroke="#CC8800" stroke-width="2" />
      <rect x="96" y="58" width="8" height="10" rx="2" fill="#FFFFFF" />
      <line x1="100" y1="60" x2="100" y2="66" stroke="#00B050" stroke-width="2" />
      <line x1="97" y1="63" x2="103" y2="63" stroke="#00B050" stroke-width="2" />
    </g>

    <!-- #acc-box -->
    <g id="${p}acc-box" class="acc-group" opacity="0">
      <g transform="translate(80, 118)">
        <rect x="0" y="0" width="40" height="28" rx="4" fill="#C99863" stroke="#8E6539" stroke-width="2" />
        <rect x="16" y="0" width="8" height="28" fill="#F8E5C8" opacity="0.9" />
        <path d="M 0 8 L 40 8" stroke="#8E6539" stroke-width="1.5" stroke-dasharray="3,2" />
        <path d="M 8 18 L 12 14 L 12 22 M 12 18 L 14 18" stroke="#5A3A1A" stroke-width="1.5" stroke-linecap="round" />
      </g>
    </g>

    <!-- #acc-sign -->
    <g id="${p}acc-sign" class="acc-group" opacity="0">
      <g transform="translate(118, 90)">
        <line x1="10" y1="28" x2="10" y2="52" stroke="#8A5A00" stroke-width="3" stroke-linecap="round" />
        <rect x="-10" y="0" width="40" height="28" rx="6" fill="#FFD166" stroke="#D49A1A" stroke-width="2.5" />
        <text x="10" y="18" text-anchor="middle" font-family="'JetBrains Mono', monospace" font-size="10" font-weight="900" fill="#6A4500">HELD</text>
      </g>
    </g>

    <!-- #acc-flashlight -->
    <g id="${p}acc-flashlight" class="acc-group" opacity="0">
      <polygon points="135,115 195,85 195,145" fill="url(#${p}beam-grad)" />
      <g transform="translate(125, 110) rotate(-10)">
        <rect x="0" y="2" width="16" height="8" rx="2" fill="#4A5568" />
        <path d="M 14 0 L 22 -3 L 22 13 L 14 10 Z" fill="#CBD5E0" stroke="#4A5568" stroke-width="1" />
        <circle cx="22" cy="5" r="3" fill="#39FF14" />
      </g>
    </g>

    <!-- #acc-glasses -->
    <g id="${p}acc-glasses" class="acc-glasses" opacity="0">
      <rect x="94" y="98" width="12" height="4" rx="1.5" fill="#39FF14" filter="url(#${p}neon-glow)" />
      <rect x="68" y="92" width="28" height="20" rx="4" fill="#03110A" stroke="#39FF14" stroke-width="2.5" filter="url(#${p}neon-glow)" />
      <rect x="104" y="92" width="28" height="20" rx="4" fill="#03110A" stroke="#39FF14" stroke-width="2.5" filter="url(#${p}neon-glow)" />
      <path id="${p}glasses-glint" class="glasses-glint" d="M 72 96 L 82 108 M 76 96 L 86 108 M 108 96 L 118 108 M 112 96 L 122 108" 
            stroke="#5CFF3A" stroke-width="1.8" stroke-linecap="round" opacity="0.9" />
    </g>
  </g>

  <!-- #fx-layer -->
  <g id="${p}fx-layer">
    <!-- Thinking dots -->
    <g id="${p}fx-dots" class="fx-item" opacity="0">
      <circle class="dot-1" cx="88" cy="50" r="3.5" fill="#BBAEFF" />
      <circle class="dot-2" cx="100" cy="44" r="4.5" fill="#A18FFF" />
      <circle class="dot-3" cx="114" cy="48" r="5.5" fill="#7B61D6" />
    </g>

    <!-- Sparkles -->
    <g id="${p}fx-sparkles" class="fx-item" opacity="0">
      <path class="spk-1" d="M 52 90 Q 56 94 60 90 Q 56 86 52 90 Z M 56 86 Q 56 94 56 94 Z" fill="#E9B949" />
      <path class="spk-2" d="M 148 85 Q 153 89 158 85 Q 153 81 148 85 Z" fill="#39FF14" />
      <path class="spk-3" d="M 98 40 Q 102 44 106 40 Q 102 36 98 40 Z" fill="#E9B949" />
    </g>

    <!-- Sweat drop -->
    <g id="${p}fx-sweat" class="fx-item" opacity="0">
      <path d="M 126 84 C 126 80, 132 80, 132 84 C 132 88, 126 88, 126 84 Z" fill="#64D2FF" stroke="#3AAEE6" stroke-width="1" />
    </g>

    <!-- Crying streaming tears (for error / red) -->
    <g id="${p}fx-tears" class="fx-item fx-tears" opacity="0">
      <ellipse class="tear-drop-l" cx="77" cy="116" rx="3.5" ry="5.5" fill="#64D2FF" />
      <ellipse class="tear-drop-r" cx="123" cy="116" rx="3.5" ry="5.5" fill="#64D2FF" />
      <path class="tear-stream-l" d="M 77 122 C 75 130, 79 138, 77 146" fill="none" stroke="#64D2FF" stroke-width="2" stroke-linecap="round" />
      <path class="tear-stream-r" d="M 123 122 C 125 130, 121 138, 123 146" fill="none" stroke="#64D2FF" stroke-width="2" stroke-linecap="round" />
      <!-- Tear puddles -->
      <ellipse cx="77" cy="150" rx="5" ry="2" fill="rgba(100, 210, 255, 0.6)" />
      <ellipse cx="123" cy="150" rx="5" ry="2" fill="rgba(100, 210, 255, 0.6)" />
    </g>

    <!-- Zzz -->
    <g id="${p}fx-zzz" class="fx-item" opacity="0">
      <text class="zzz-1" x="135" y="70" font-family="'Pixelify Sans', monospace" font-size="12" font-weight="bold" fill="#7B61D6">z</text>
      <text class="zzz-2" x="145" y="56" font-family="'Pixelify Sans', monospace" font-size="15" font-weight="bold" fill="#9E86FF">Z</text>
      <text class="zzz-3" x="157" y="40" font-family="'Pixelify Sans', monospace" font-size="18" font-weight="bold" fill="#BBAEFF">Z</text>
    </g>

    <!-- Shield -->
    <g id="${p}fx-shield" class="fx-item" opacity="0">
      <path d="M 100 85 L 126 95 C 126 122, 100 138, 100 138 C 100 138, 74 122, 74 95 Z" 
            fill="url(#${p}shield-grad)" 
            stroke="#FF4B4B" 
            stroke-width="3" 
            stroke-linejoin="round" />
      <line x1="100" y1="98" x2="100" y2="114" stroke="#FFFFFF" stroke-width="3" stroke-linecap="round" />
      <circle cx="100" cy="122" r="2" fill="#FFFFFF" />
    </g>

    <!-- Bandage -->
    <g id="${p}fx-bandage" class="fx-item" opacity="0">
      <g transform="translate(68, 86) rotate(-25)">
        <rect x="0" y="0" width="18" height="9" rx="2" fill="#F4C2B0" stroke="#D99B88" stroke-width="1.5" />
        <rect x="5" y="0" width="8" height="9" fill="#FFF0EB" />
      </g>
    </g>

    <!-- Clock -->
    <g id="${p}fx-clock" class="fx-item" opacity="0">
      <g transform="translate(138, 50)">
        <circle cx="12" cy="12" r="12" fill="#FBF6EA" stroke="#B5763F" stroke-width="2.5" />
        <line class="clock-minute" x1="12" y1="12" x2="12" y2="4" stroke="#3B2F2A" stroke-width="2" stroke-linecap="round" />
        <line class="clock-hour" x1="12" y1="12" x2="18" y2="12" stroke="#B5763F" stroke-width="2" stroke-linecap="round" />
      </g>
    </g>
  </g>
</svg>
`;
}
