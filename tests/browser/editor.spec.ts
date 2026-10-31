import { test, expect, Page } from '@playwright/test';

async function native(page: Page, cmd: string, args: object = {}) {
  return page.evaluate(({cmd,args}) => (window as any).__TAURI_INTERNALS__.invoke(cmd,args), {cmd,args});
}

test('real Rust document: draw, layers, four frames, playback, undo and project round-trip', async ({page}) => {
  const errors: string[] = [];
  const projectName = `browser-${Date.now()}`;
  page.on('pageerror', e => errors.push(e.message));
  await page.addInitScript(() => {
    (window as any).__TAURI_INTERNALS__ = { invoke: async (cmd: string,args: object={}) => {
      const result = await fetch('http://127.0.0.1:1421/invoke',{method:'POST',headers:{'Content-Type':'application/json'},body:JSON.stringify({cmd,args})}).then(r=>r.json());
      if (result.error) throw new Error(result.error);
      return result.ok;
    }};
  });
  await page.goto('/');
  await expect(page.getByText('64 × 64 px',{exact:true})).toBeVisible();
  const art=page.locator('canvas[aria-label="Pixel canvas"]');
  const box=await art.boundingBox();
  expect(box).not.toBeNull();
  await page.mouse.move(box!.x+4,box!.y+4);
  await page.mouse.down();
  await page.mouse.move(box!.x+box!.width-4,box!.y+box!.height-4,{steps:10});
  await page.mouse.up();
  await expect.poll(async()=>(await native(page,'document_snapshot')).revision).toBe(1);
  let doc=await native(page,'document_snapshot');
  expect(doc.pixels.slice(0,4)).toEqual([240,242,255,255]);
  expect(doc.pixels.slice(-4)).toEqual([240,242,255,255]);
  await page.getByRole('button',{name:'Undo',exact:true}).click();
  await expect.poll(async()=>(await native(page,'document_snapshot')).pixels.every((v:number)=>v===0)).toBe(true);
  await page.getByRole('button',{name:'Redo',exact:true}).click();
  await expect.poll(async()=>(await native(page,'document_snapshot')).revision).toBe(3);
  await page.getByRole('button',{name:'+ Add',exact:true}).click();
  await expect.poll(async()=>(await native(page,'document_snapshot')).layers.length).toBe(2);
  for (let n=2;n<=4;n++) {
    await page.getByRole('button',{name:'+ Frame',exact:true}).click();
    await expect.poll(async()=>(await native(page,'document_snapshot')).frames.length).toBe(n);
  }
  await page.getByRole('button',{name:'Play',exact:true}).click();
  const first=(await native(page,'document_snapshot')).active_frame_id;
  await expect.poll(async()=>(await native(page,'document_snapshot')).active_frame_id).not.toBe(first);
  await page.getByRole('button',{name:'Pause',exact:true}).click();
  await page.getByRole('button',{name:'Previous frame',exact:true}).click();
  await page.getByRole('textbox',{name:'Project name'}).fill(projectName);
  await page.getByRole('button',{name:'Save',exact:true}).click();
  await expect(page.getByRole('status')).toContainText('Project saved');
  const saved=await native(page,'document_snapshot');
  expect(saved.dirty).toBe(false);
  await page.getByRole('button',{name:'Export sheet',exact:true}).click();
  await expect(page.getByRole('status')).toContainText('Exported to');
  await page.getByRole('combobox',{name:'New document size'}).selectOption('16');
  await page.getByRole('button',{name:'New',exact:true}).click();
  await expect(page.getByText('16 × 16 px',{exact:true})).toBeVisible();
  page.on('dialog',dialog=>dialog.accept());
  await page.getByRole('textbox',{name:'Project name'}).fill(projectName);
  await page.getByRole('button',{name:'Load',exact:true}).click();
  await expect(page.getByText('64 × 64 px',{exact:true})).toBeVisible();
  doc=await native(page,'document_snapshot');
  expect(doc.pixels).toEqual(saved.pixels);
  expect(doc.layers).toEqual(saved.layers);
  expect(doc.frames).toEqual(saved.frames);
  expect(errors).toEqual([]);
  await page.screenshot({path:'work/editor-browser.png',fullPage:true});
});
