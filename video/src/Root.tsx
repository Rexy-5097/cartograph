import React from 'react'
import { Composition } from 'remotion'
import './fonts'
import { Commercial, COMMERCIAL_FRAMES } from './commercial/Commercial'
import { FPS, HEIGHT, WIDTH } from './theme'
import { Walkthrough, WALKTHROUGH_FRAMES } from './Walkthrough'

export const Root: React.FC = () => (
  <>
    <Composition id="Walkthrough" component={Walkthrough} durationInFrames={WALKTHROUGH_FRAMES} fps={FPS} width={WIDTH} height={HEIGHT} />
    <Composition id="Commercial" component={Commercial} durationInFrames={COMMERCIAL_FRAMES} fps={FPS} width={WIDTH} height={HEIGHT} />
  </>
)
