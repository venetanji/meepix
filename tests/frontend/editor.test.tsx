import { cleanup, fireEvent, render, screen, waitFor } from '@testing-library/react';
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest';
import App, { type DocumentBackend, type Snapshot, mapPoint } from '../../src/App';

beforeEach(() => { vi.stubGlobal('PointerEvent', MouseEvent); vi.spyOn(HTMLCanvasElement.prototype, 'getContext').mockImplementation(() => ({ createImageData: (w:number,h:number) => ({data:new Uint8ClampedArray(w*h*4)}), putImageData: () => {} }) as unknown as CanvasRenderingContext2D); });
afterEach(cleanup);
const snapshot: Snapshot = { document_id:'doc', revision:0, width:16, height:16, layers:[{id:'layer',name:'Layer 1',visible:true}], frames:[{id:'frame',duration_ms:100}], active_layer_id:'layer', active_frame_id:'frame', palette:[[0,0,0,255]], pixels:Array(16*16*4).fill(0), can_undo:true, can_redo:false, dirty:false };
function backend(): DocumentBackend { return { snapshot:vi.fn().mockResolvedValue(snapshot), create:vi.fn().mockResolvedValue(snapshot), command:vi.fn().mockResolvedValue({...snapshot,revision:1,dirty:true}), save:vi.fn().mockResolvedValue(snapshot), load:vi.fn().mockResolvedValue(snapshot), export:vi.fn().mockResolvedValue('sheet.png'), models:vi.fn().mockResolvedValue({configured:false,models:[]}), submit:vi.fn(), poll:vi.fn(), reject:vi.fn(), accept:vi.fn(), frameSnapshot:vi.fn().mockResolvedValue(snapshot.pixels) }; }
describe('editor', () => {
 it('maps edges using the displayed canvas bounds', () => { const rect={left:10,top:20,width:160,height:160}; expect(mapPoint(10,20,rect,16,16)).toEqual({x:0,y:0}); expect(mapPoint(169.9,179.9,rect,16,16)).toEqual({x:15,y:15}); expect(mapPoint(170,180,rect,16,16)).toBeNull(); });
 it('sends a complete stroke as one native transaction and uses returned dirty state', async () => { const api=backend(); render(<App backend={api}/>); const canvas=await screen.findByLabelText('Pixel canvas'); vi.spyOn(canvas,'getBoundingClientRect').mockReturnValue({left:0,top:0,width:160,height:160} as DOMRect); fireEvent.pointerDown(canvas,{clientX:1,clientY:1,pointerId:1,button:0}); fireEvent.pointerMove(canvas,{clientX:159,clientY:159,pointerId:1}); fireEvent.pointerUp(canvas,{clientX:159,clientY:159,pointerId:1}); await waitFor(()=>expect(api.command).toHaveBeenCalledWith(0,expect.objectContaining({type:'paint',points:[{x:0,y:0},{x:15,y:15}]}))); expect(await screen.findByText('Unsaved changes')).toBeTruthy(); });
 it('uses transparent paint for erasing', async () => { const api=backend(); render(<App backend={api}/>); const canvas=await screen.findByLabelText('Pixel canvas'); vi.spyOn(canvas,'getBoundingClientRect').mockReturnValue({left:0,top:0,width:160,height:160} as DOMRect); fireEvent.click(screen.getByRole('button',{name:'Eraser'})); fireEvent.pointerDown(canvas,{clientX:1,clientY:1,button:0}); fireEvent.pointerUp(canvas,{clientX:1,clientY:1}); await waitFor(()=>expect(api.command).toHaveBeenCalledWith(0,expect.objectContaining({type:'paint',color:[0,0,0,0]}))); });
 it('sends layer selection and undo through the backend', async () => { const api=backend(); render(<App backend={api}/>); await screen.findByText('Layer 1'); fireEvent.click(screen.getByRole('button',{name:'Layer 1'})); await waitFor(()=>expect(api.command).toHaveBeenCalledWith(0,{type:'select_layer',layer_id:'layer'})); fireEvent.click(screen.getByRole('button',{name:'Undo'})); await waitFor(()=>expect(api.command).toHaveBeenCalledWith(1,{type:'undo'})); });
});

it('keeps the stroke color and target captured at pointer down', async () => {
  const api = backend();
  render(<App backend={api}/>);
  const canvas = await screen.findByLabelText('Pixel canvas');
  vi.spyOn(canvas, 'getBoundingClientRect').mockReturnValue({left:0,top:0,width:160,height:160} as DOMRect);
  fireEvent.pointerDown(canvas, {clientX:1,clientY:1,button:0});
  fireEvent.click(screen.getByRole('button', {name:'Eraser'}));
  fireEvent.pointerUp(canvas, {clientX:11,clientY:11});
  await waitFor(() => expect(api.command).toHaveBeenCalledWith(0, expect.objectContaining({type:'paint', layer_id:'layer', frame_id:'frame', color:[240,242,255,255]})));
});

it('disables frame and layer changes while an edit is pending', async () => {
  const api = backend();
  let finish!: (value: Snapshot) => void;
  vi.mocked(api.command).mockReturnValue(new Promise(resolve => {finish = resolve;}));
  render(<App backend={api}/>);
  await screen.findByText('Layer 1');
  fireEvent.click(screen.getByRole('button', {name:'Layer 1'}));
  await waitFor(() => expect((screen.getByRole('button', {name:'Layer 1'}) as HTMLButtonElement).disabled).toBe(true));
  expect((screen.getByRole('button', {name:'01 100 ms'}) as HTMLButtonElement).disabled).toBe(true);
  finish(snapshot);
  await waitFor(() => expect((screen.getByRole('button', {name:'Layer 1'}) as HTMLButtonElement).disabled).toBe(false));
});
