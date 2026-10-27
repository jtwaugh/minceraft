// Textures the scene's model atlas is packed from.
import { RAFT_TEXTURES } from './raftmodel.js';
import { ITEM_TEXTURES } from './itemmodel.js';
import { HELD_TEXTURES } from './helditem.js';

export const SCENE_TEXTURES = [...RAFT_TEXTURES, ...ITEM_TEXTURES, ...HELD_TEXTURES];
