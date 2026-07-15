import { act, fireEvent, render, screen, waitFor } from '@testing-library/react';
import { afterAll, beforeAll, describe, expect, it, vi } from 'vitest';

import PhotoViewer from './PhotoViewer';
import type { PhotoItem } from '../types/album';

class TestPointerEvent extends MouseEvent {
  readonly pointerId: number;
  readonly pointerType: string;

  constructor(type: string, init: PointerEventInit = {}) {
    super(type, init);
    this.pointerId = init.pointerId ?? 0;
    this.pointerType = init.pointerType ?? '';
  }
}

beforeAll(() => {
  vi.stubGlobal('PointerEvent', TestPointerEvent);
});

afterAll(() => {
  vi.unstubAllGlobals();
});

const emptyExif = {
  cameraMake: null,
  cameraModel: null,
  focalLength: null,
  aperture: null,
  shutterSpeed: null,
  iso: null,
};

const photos: PhotoItem[] = [
  {
    filename: 'first.jpg',
    thumbnailUrl: '/first-thumb.jpg',
    originalUrl: '/first.jpg',
    exif: emptyExif,
  },
  {
    filename: 'second.jpg',
    thumbnailUrl: '/second-thumb.jpg',
    originalUrl: '/second.jpg',
    exif: emptyExif,
  },
];

function getStage(imageName = 'first.jpg') {
  return screen.getByRole('img', { name: imageName }).parentElement?.parentElement?.parentElement as HTMLElement;
}

function getTrack() {
  return getStage().children[1] as HTMLElement;
}

function getMedia() {
  return screen.getByRole('img', { name: 'first.jpg' }).parentElement as HTMLElement;
}

describe('PhotoViewer gestures', () => {
  it('opens the requested photo and does not close when it is clicked', () => {
    const onClose = vi.fn();
    render(<PhotoViewer photos={photos} initialIndex={1} onClose={onClose} />);
    const image = screen.getByRole('img', { name: 'second.jpg' });
    const stage = getStage('second.jpg');

    expect(image.getAttribute('src')).toBe('/second-thumb.jpg');
    fireEvent.pointerDown(image, { pointerId: 1, pointerType: 'mouse', button: 0, clientX: 100, clientY: 100 });
    fireEvent.pointerUp(stage, { pointerId: 1, pointerType: 'mouse', clientX: 100, clientY: 100 });

    expect(onClose).not.toHaveBeenCalled();
    expect(image.parentElement?.style.transform).toContain('scale(2)');
  });

  it('toggles zoom around the clicked point with a mouse', () => {
    render(<PhotoViewer photos={photos} initialIndex={0} onClose={vi.fn()} />);
    const stage = getStage();
    const image = screen.getByRole('img', { name: 'first.jpg' });
    const originalImage = stage.querySelector('img[src="/first.jpg"]') as HTMLImageElement;
    Object.defineProperties(stage, {
      clientWidth: { configurable: true, value: 800 },
      clientHeight: { configurable: true, value: 600 },
    });
    Object.defineProperties(originalImage, {
      clientWidth: { configurable: true, value: 600 },
      clientHeight: { configurable: true, value: 500 },
    });
    vi.spyOn(stage, 'getBoundingClientRect').mockReturnValue({
      x: 0,
      y: 0,
      top: 0,
      right: 800,
      bottom: 600,
      left: 0,
      width: 800,
      height: 600,
      toJSON: () => ({}),
    });

    fireEvent.pointerDown(image, { pointerId: 1, pointerType: 'mouse', button: 0, clientX: 600, clientY: 300 });
    fireEvent.pointerUp(stage, { pointerId: 1, pointerType: 'mouse', clientX: 600, clientY: 300 });
    expect(getMedia().style.transform).toContain('translate3d(-200px, 0px, 0) scale(2)');

    fireEvent.pointerDown(image, { pointerId: 2, pointerType: 'mouse', button: 0, clientX: 600, clientY: 300 });
    fireEvent.pointerUp(stage, { pointerId: 2, pointerType: 'mouse', clientX: 600, clientY: 300 });
    expect(getMedia().style.transform).toContain('scale(1)');
  });

  it('prevents the wheel event default while zooming', () => {
    render(<PhotoViewer photos={photos} initialIndex={0} onClose={vi.fn()} />);
    const stage = getStage();
    const wheelEvent = new WheelEvent('wheel', {
      bubbles: true,
      cancelable: true,
      clientX: 100,
      clientY: 100,
      deltaY: -100,
    });

    act(() => {
      stage.dispatchEvent(wheelEvent);
    });

    expect(wheelEvent.defaultPrevented).toBe(true);
    expect(getMedia().style.transform).toContain('scale(1.25)');
  });

  it('closes only when an empty stage area is clicked without click-through', () => {
    const onClose = vi.fn();
    const underlayClick = vi.fn();
    render(
      <>
        <button onClick={underlayClick}>Underlay action</button>
        <PhotoViewer photos={photos} initialIndex={0} onClose={onClose} />
      </>
    );
    const stage = getStage();
    const image = screen.getByRole('img', { name: 'first.jpg' });

    fireEvent.pointerDown(image, { pointerId: 1, pointerType: 'mouse', button: 0, clientX: 100, clientY: 100 });
    fireEvent.pointerUp(stage, { pointerId: 1, pointerType: 'mouse', clientX: 100, clientY: 100 });
    fireEvent.click(image);
    expect(onClose).not.toHaveBeenCalled();

    fireEvent.pointerDown(stage, { pointerId: 2, pointerType: 'mouse', button: 0, clientX: 10, clientY: 10 });
    fireEvent.pointerUp(stage, { pointerId: 2, pointerType: 'mouse', clientX: 10, clientY: 10 });
    expect(onClose).not.toHaveBeenCalled();
    fireEvent.click(stage);

    expect(onClose).toHaveBeenCalledOnce();
    expect(underlayClick).not.toHaveBeenCalled();
  });

  it('toggles zoom after a double tap on the image', () => {
    render(<PhotoViewer photos={photos} initialIndex={0} onClose={vi.fn()} />);
    const stage = getStage();
    const image = screen.getByRole('img', { name: 'first.jpg' });

    fireEvent.pointerDown(image, { pointerId: 1, pointerType: 'touch', clientX: 100, clientY: 100 });
    fireEvent.pointerUp(stage, { pointerId: 1, pointerType: 'touch', clientX: 100, clientY: 100 });
    fireEvent.pointerDown(image, { pointerId: 2, pointerType: 'touch', clientX: 104, clientY: 102 });
    fireEvent.pointerUp(stage, { pointerId: 2, pointerType: 'touch', clientX: 104, clientY: 102 });

    expect(getMedia().style.transform).toContain('scale(2)');
  });

  it('ignores vertical touch gestures', () => {
    render(<PhotoViewer photos={photos} initialIndex={0} onClose={vi.fn()} />);
    const stage = getStage();
    const image = screen.getByRole('img', { name: 'first.jpg' });

    fireEvent.pointerDown(image, { pointerId: 1, pointerType: 'touch', clientX: 100, clientY: 80 });
    fireEvent.pointerMove(stage, { pointerId: 1, pointerType: 'touch', clientX: 108, clientY: 180 });
    fireEvent.pointerUp(stage, { pointerId: 1, pointerType: 'touch', clientX: 108, clientY: 180 });

    expect(screen.getByRole('img', { name: 'first.jpg' })).not.toBeNull();
  });

  it('does not navigate past the first photo', () => {
    render(<PhotoViewer photos={photos} initialIndex={0} onClose={vi.fn()} />);
    const stage = getStage();
    const image = screen.getByRole('img', { name: 'first.jpg' });

    fireEvent.pointerDown(image, { pointerId: 1, pointerType: 'touch', clientX: 80, clientY: 100 });
    fireEvent.pointerMove(stage, { pointerId: 1, pointerType: 'touch', clientX: 180, clientY: 102 });
    fireEvent.pointerUp(stage, { pointerId: 1, pointerType: 'touch', clientX: 180, clientY: 102 });

    expect(screen.getByRole('img', { name: 'first.jpg' })).not.toBeNull();
  });

  it('renders when EXIF data is missing', () => {
    const photoWithoutExif: PhotoItem = {
      ...photos[0],
      exif: {
        cameraMake: null,
        cameraModel: null,
        focalLength: null,
        aperture: null,
        shutterSpeed: null,
        iso: null,
      },
    };

    render(<PhotoViewer photos={[photoWithoutExif]} initialIndex={0} onClose={vi.fn()} />);

    expect(screen.getByRole('img', { name: 'first.jpg' })).not.toBeNull();
    expect(screen.getByText('1 / 1')).not.toBeNull();
  });

  it('reveals a cached image after navigating to it', async () => {
    const complete = vi.spyOn(HTMLImageElement.prototype, 'complete', 'get').mockReturnValue(true);
    const naturalWidth = vi.spyOn(HTMLImageElement.prototype, 'naturalWidth', 'get').mockReturnValue(1200);

    const { container } = render(<PhotoViewer photos={photos} initialIndex={0} onClose={vi.fn()} />);
    fireEvent.click(screen.getByRole('button', { name: 'Next photo' }));

    await waitFor(() => {
      const original = container.querySelector('img[src="/second.jpg"]') as HTMLImageElement | null;
      expect(original?.style.display).toBe('block');
    });

    complete.mockRestore();
    naturalWidth.mockRestore();
  });

  it('switches photos after a horizontal touch swipe', () => {
    render(<PhotoViewer photos={photos} initialIndex={0} onClose={vi.fn()} />);
    const stage = getStage();
    const image = screen.getByRole('img', { name: 'first.jpg' });

    fireEvent.pointerDown(image, { pointerId: 1, pointerType: 'touch', clientX: 180, clientY: 100 });
    fireEvent.pointerMove(stage, { pointerId: 1, pointerType: 'touch', clientX: 80, clientY: 104 });
    fireEvent.pointerUp(stage, { pointerId: 1, pointerType: 'touch', clientX: 80, clientY: 104 });
    fireEvent.transitionEnd(getTrack(), { propertyName: 'transform' });

    expect(screen.getByText('2 / 2')).not.toBeNull();
  });
});
