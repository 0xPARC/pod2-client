import test from 'ava'
import { MainPod } from '../index.js'
import serializedMainPod from './mainpod.json' assert { type: 'json' }

test('deserialize main pod', (t) => {
  const mainPod = MainPod.deserialize(JSON.stringify(serializedMainPod))
  t.is(mainPod.verify(), true)
})
