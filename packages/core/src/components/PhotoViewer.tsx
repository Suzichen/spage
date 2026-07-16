import React, { useState, useEffect, useCallback, useMemo, useRef } from 'react';
import type { PhotoItem, ExifData } from '../types/album';

interface PhotoViewerProps {
  photos: PhotoItem[];
  initialIndex: number;
  onClose: () => void;
}

// Check if a URL points to a HEIC file (browsers can't display HEIC natively)
function isHeicFile(url: string): boolean {
  return url.toLowerCase().endsWith('.heic');
}

// Get the display URL for a photo (use thumbnail for HEIC since browsers can't display it)
function getDisplayUrl(photo: PhotoItem): string {
  return isHeicFile(photo.originalUrl) ? photo.thumbnailUrl : photo.originalUrl;
}

// Pure function for navigation index calculation (exported for testing)
export function navigatePhoto(
  index: number,
  direction: 'prev' | 'next',
  total: number
): number {
  if (direction === 'prev' && index > 0) return index - 1;
  if (direction === 'next' && index < total - 1) return index + 1;
  return index;
}

const MIN_SCALE = 1;
const MAX_SCALE = 4;
const SWIPE_THRESHOLD = 50;
const SWIPE_LOCK_THRESHOLD = 10;
const SWIPE_VELOCITY_THRESHOLD = 0.45;
const EDGE_RESISTANCE = 0.28;

interface Point {
  x: number;
  y: number;
}

function clamp(value: number, min: number, max: number): number {
  return Math.min(max, Math.max(min, value));
}

export function getSwipeDirection(
  deltaX: number,
  deltaY: number,
  threshold = SWIPE_THRESHOLD
): 'prev' | 'next' | null {
  if (Math.abs(deltaX) < threshold || Math.abs(deltaX) <= Math.abs(deltaY)) return null;
  return deltaX > 0 ? 'prev' : 'next';
}

const PhotoViewer: React.FC<PhotoViewerProps> = ({ photos, initialIndex, onClose }) => {
  const [currentIndex, setCurrentIndex] = useState(initialIndex);
  const [imageLoaded, setImageLoaded] = useState(false);
  const [scale, setScale] = useState(MIN_SCALE);
  const [position, setPosition] = useState<Point>({ x: 0, y: 0 });
  const [swipeOffset, setSwipeOffset] = useState(0);
  const [isInteracting, setIsInteracting] = useState(false);
  const [isSlideTransitioning, setIsSlideTransitioning] = useState(false);
  const stageRef = useRef<HTMLDivElement>(null);
  const mediaRef = useRef<HTMLDivElement>(null);
  const imageRef = useRef<HTMLImageElement>(null);
  const activePointerRef = useRef<number | null>(null);
  const scaleRef = useRef(MIN_SCALE);
  const positionRef = useRef<Point>({ x: 0, y: 0 });
  const lastTouchTapRef = useRef<{ time: number; point: Point } | null>(null);
  const pendingNavigationRef = useRef<'prev' | 'next' | null>(null);
  const gestureRef = useRef({
    startPoint: { x: 0, y: 0 },
    startPosition: { x: 0, y: 0 },
    pointerType: '',
    startedOnImage: false,
    moved: false,
    axis: null as 'horizontal' | 'vertical' | null,
    startTime: 0,
  });

  const currentPhoto = photos[currentIndex];
  const previousPhoto = currentIndex > 0 ? photos[currentIndex - 1] : null;
  const nextPhoto = currentIndex < photos.length - 1 ? photos[currentIndex + 1] : null;
  const total = photos.length;

  // Get the URL to display (use thumbnail for HEIC files)
  const displayUrl = useMemo(() => getDisplayUrl(currentPhoto), [currentPhoto]);

  const applyTransform = useCallback((nextScale: number, nextPosition: Point) => {
    const stage = stageRef.current;
    const image = imageRef.current;
    const normalizedScale = clamp(nextScale, MIN_SCALE, MAX_SCALE);
    let normalizedPosition = { x: 0, y: 0 };

    if (stage && image && normalizedScale > MIN_SCALE) {
      const maxX = Math.max(0, (image.clientWidth * normalizedScale - stage.clientWidth) / 2);
      const maxY = Math.max(0, (image.clientHeight * normalizedScale - stage.clientHeight) / 2);
      normalizedPosition = {
        x: clamp(nextPosition.x, -maxX, maxX),
        y: clamp(nextPosition.y, -maxY, maxY),
      };
    }

    scaleRef.current = normalizedScale;
    positionRef.current = normalizedPosition;
    setScale(normalizedScale);
    setPosition(normalizedPosition);
  }, []);

  const resetTransform = useCallback(() => {
    applyTransform(MIN_SCALE, { x: 0, y: 0 });
    setSwipeOffset(0);
    activePointerRef.current = null;
    lastTouchTapRef.current = null;
    pendingNavigationRef.current = null;
    setIsSlideTransitioning(false);
  }, [applyTransform]);

  const handleImageLoad = useCallback(() => {
    setImageLoaded(true);
  }, []);

  // Reset the transform, then account for images already fulfilled by cache.
  useEffect(() => {
    setImageLoaded(false);
    resetTransform();

    const frame = requestAnimationFrame(() => {
      const image = imageRef.current;
      if (image?.complete && image.naturalWidth > 0) {
        setImageLoaded(true);
      }
    });

    return () => cancelAnimationFrame(frame);
  }, [currentIndex, displayUrl, resetTransform]);

  // Lock body scroll on mount, restore on unmount
  useEffect(() => {
    const originalOverflow = document.body.style.overflow;
    document.body.style.overflow = 'hidden';
    return () => {
      document.body.style.overflow = originalOverflow;
    };
  }, []);

  const handlePrev = useCallback(() => {
    setCurrentIndex((i) => navigatePhoto(i, 'prev', total));
  }, [total]);

  const handleNext = useCallback(() => {
    setCurrentIndex((i) => navigatePhoto(i, 'next', total));
  }, [total]);

  const zoomTo = useCallback((nextScale: number, focalPoint?: Point) => {
    const normalizedScale = clamp(nextScale, MIN_SCALE, MAX_SCALE);
    const currentScale = scaleRef.current;
    const stage = stageRef.current;

    if (!focalPoint || !stage || normalizedScale === MIN_SCALE) {
      applyTransform(normalizedScale, positionRef.current);
      return;
    }

    const rect = stage.getBoundingClientRect();
    const focalOffset = {
      x: focalPoint.x - rect.left - rect.width / 2,
      y: focalPoint.y - rect.top - rect.height / 2,
    };
    const ratio = normalizedScale / currentScale;
    const currentPosition = positionRef.current;

    applyTransform(normalizedScale, {
      x: focalOffset.x - (focalOffset.x - currentPosition.x) * ratio,
      y: focalOffset.y - (focalOffset.y - currentPosition.y) * ratio,
    });
  }, [applyTransform]);

  const toggleZoom = useCallback((focalPoint: Point) => {
    zoomTo(scaleRef.current > MIN_SCALE ? MIN_SCALE : 2, focalPoint);
  }, [zoomTo]);

  const handlePointerDown = (e: React.PointerEvent<HTMLDivElement>) => {
    if (e.pointerType === 'mouse' && e.button !== 0) return;
    if (activePointerRef.current !== null) return;

    if (isSlideTransitioning) return;

    activePointerRef.current = e.pointerId;
    gestureRef.current = {
      startPoint: { x: e.clientX, y: e.clientY },
      startPosition: positionRef.current,
      pointerType: e.pointerType,
      startedOnImage: mediaRef.current?.contains(e.target as Node) ?? false,
      moved: false,
      axis: null,
      startTime: performance.now(),
    };
    if (scaleRef.current > MIN_SCALE) setIsInteracting(true);
  };

  const handlePointerMove = (e: React.PointerEvent<HTMLDivElement>) => {
    if (activePointerRef.current !== e.pointerId) return;

    const deltaX = e.clientX - gestureRef.current.startPoint.x;
    const deltaY = e.clientY - gestureRef.current.startPoint.y;
    const gesture = gestureRef.current;
    if (Math.abs(deltaX) > 8 || Math.abs(deltaY) > 8) {
      gesture.moved = true;
    }

    if (scaleRef.current > MIN_SCALE) {
      e.preventDefault();
      applyTransform(scaleRef.current, {
        x: gesture.startPosition.x + deltaX,
        y: gesture.startPosition.y + deltaY,
      });
      return;
    }

    if (gesture.pointerType !== 'touch') return;

    if (!gesture.axis && Math.hypot(deltaX, deltaY) >= SWIPE_LOCK_THRESHOLD) {
      gesture.axis = Math.abs(deltaX) > Math.abs(deltaY) * 1.2 ? 'horizontal' : 'vertical';
      if (gesture.axis === 'horizontal') setIsInteracting(true);
    }

    if (gesture.axis !== 'horizontal') return;

    e.preventDefault();
    const pullingPastStart = currentIndex === 0 && deltaX > 0;
    const pullingPastEnd = currentIndex === total - 1 && deltaX < 0;
    setSwipeOffset((pullingPastStart || pullingPastEnd) ? deltaX * EDGE_RESISTANCE : deltaX);
  };

  const finishPointer = (e: React.PointerEvent<HTMLDivElement>, canceled = false) => {
    if (activePointerRef.current !== e.pointerId) return;

    const gesture = gestureRef.current;
    const point = { x: e.clientX, y: e.clientY };
    const deltaX = point.x - gesture.startPoint.x;
    const deltaY = point.y - gesture.startPoint.y;

    activePointerRef.current = null;
    setIsInteracting(false);

    if (canceled) {
      setSwipeOffset(0);
      return;
    }

    if (scaleRef.current === MIN_SCALE && gesture.pointerType === 'touch' && gesture.moved) {
      if (gesture.axis !== 'horizontal') return;

      const elapsed = Math.max(1, performance.now() - gesture.startTime);
      const velocity = Math.abs(deltaX) / elapsed;
      const direction = getSwipeDirection(
        deltaX,
        deltaY,
        velocity >= SWIPE_VELOCITY_THRESHOLD ? 24 : SWIPE_THRESHOLD
      );
      const canNavigate = direction === 'prev'
        ? currentIndex > 0
        : direction === 'next' && currentIndex < total - 1;

      pendingNavigationRef.current = canNavigate ? direction : null;
      setIsSlideTransitioning(true);
      if (canNavigate && direction) {
        const stageWidth = stageRef.current?.clientWidth || window.innerWidth;
        setSwipeOffset(direction === 'next' ? -stageWidth : stageWidth);
      } else {
        setSwipeOffset(0);
      }
      return;
    }

    if (gesture.moved) return;

    if (!gesture.startedOnImage) return;

    if (gesture.pointerType === 'mouse') {
      toggleZoom(point);
      return;
    }

    if (gesture.pointerType === 'touch') {
      const now = performance.now();
      const previousTap = lastTouchTapRef.current;
      const isDoubleTap = previousTap
        && now - previousTap.time <= 300
        && Math.hypot(point.x - previousTap.point.x, point.y - previousTap.point.y) <= 24;

      if (isDoubleTap) {
        lastTouchTapRef.current = null;
        toggleZoom(point);
      } else {
        lastTouchTapRef.current = { time: now, point };
      }
    }
  };

  const handleSlideTransitionEnd = (e: React.TransitionEvent<HTMLDivElement>) => {
    if (e.target !== e.currentTarget) return;

    const direction = pendingNavigationRef.current;
    pendingNavigationRef.current = null;
    setIsSlideTransitioning(false);
    setSwipeOffset(0);

    if (direction === 'prev') handlePrev();
    if (direction === 'next') handleNext();
  };

  useEffect(() => {
    const stage = stageRef.current;
    if (!stage) return;

    const handleWheel = (e: WheelEvent) => {
      e.preventDefault();
      zoomTo(
        scaleRef.current + (e.deltaY < 0 ? 0.25 : -0.25),
        { x: e.clientX, y: e.clientY }
      );
    };

    stage.addEventListener('wheel', handleWheel, { passive: false });
    return () => stage.removeEventListener('wheel', handleWheel);
  }, [zoomTo]);

  // Keyboard event listener
  useEffect(() => {
    const handleKeyDown = (e: KeyboardEvent) => {
      switch (e.key) {
        case 'Escape':
          onClose();
          break;
        case 'ArrowLeft':
          handlePrev();
          break;
        case 'ArrowRight':
          handleNext();
          break;
        case '+':
        case '=':
          zoomTo(scaleRef.current + 0.5);
          break;
        case '-':
          zoomTo(scaleRef.current - 0.5);
          break;
        case '0':
          zoomTo(MIN_SCALE);
          break;
      }
    };

    window.addEventListener('keydown', handleKeyDown);
    return () => window.removeEventListener('keydown', handleKeyDown);
  }, [onClose, handlePrev, handleNext, zoomTo]);

  const handleStageClick = (e: React.MouseEvent<HTMLDivElement>) => {
    e.preventDefault();
    e.stopPropagation();
    onClose();
  };

  return (
    <div
      className="fixed inset-0 z-50 flex items-center justify-center"
      style={{ backgroundColor: 'rgba(0, 0, 0, 0.9)' }}
    >
      {/* Close button */}
      <button
        onClick={onClose}
        className="absolute top-4 right-4 z-50 text-white/70 hover:text-white transition-colors"
        style={{ background: 'none', border: 'none', cursor: 'pointer', fontSize: '28px', lineHeight: 1 }}
        aria-label="Close"
      >
        ✕
      </button>

      {/* Previous button */}
      {currentIndex > 0 && (
        <button
          onClick={(e) => { e.stopPropagation(); handlePrev(); }}
          className="absolute left-4 z-50 text-white/60 hover:text-white transition-colors"
          style={{
            background: 'rgba(255,255,255,0.1)',
            border: 'none',
            cursor: 'pointer',
            fontSize: '32px',
            lineHeight: 1,
            padding: '12px 16px',
            borderRadius: '8px',
            backdropFilter: 'blur(4px)',
          }}
          aria-label="Previous photo"
        >
          ‹
        </button>
      )}

      {/* Next button */}
      {currentIndex < total - 1 && (
        <button
          onClick={(e) => { e.stopPropagation(); handleNext(); }}
          className="absolute right-4 z-50 text-white/60 hover:text-white transition-colors"
          style={{
            background: 'rgba(255,255,255,0.1)',
            border: 'none',
            cursor: 'pointer',
            fontSize: '32px',
            lineHeight: 1,
            padding: '12px 16px',
            borderRadius: '8px',
            backdropFilter: 'blur(4px)',
          }}
          aria-label="Next photo"
        >
          ›
        </button>
      )}

      {/* Photo container */}
      <div
        ref={stageRef}
        className="relative flex h-full w-full items-center justify-center overflow-hidden"
        onClick={handleStageClick}
        onPointerDown={handlePointerDown}
        onPointerMove={handlePointerMove}
        onPointerUp={(e) => finishPointer(e)}
        onPointerCancel={(e) => finishPointer(e, true)}
        style={{
          touchAction: 'none',
          cursor: scale > MIN_SCALE ? (isInteracting ? 'grabbing' : 'grab') : 'default',
        }}
      >
        <PhotoSlide
          photo={previousPhoto}
          offset={`calc(-100% + ${swipeOffset}px)`}
          transitioning={isSlideTransitioning}
        />

        <div
          className="absolute inset-0 flex items-center justify-center"
          onTransitionEnd={handleSlideTransitionEnd}
          style={{
            transform: `translate3d(${scale === MIN_SCALE ? swipeOffset : 0}px, 0, 0)`,
            transition: isSlideTransitioning ? 'transform 220ms ease-out' : 'none',
            willChange: 'transform',
          }}
        >
          <div
            ref={mediaRef}
            className="relative flex items-center justify-center"
            onClick={(e) => e.stopPropagation()}
            style={{
              transform: `translate3d(${position.x}px, ${position.y}px, 0) scale(${scale})`,
              transition: isInteracting ? 'none' : 'transform 180ms ease-out',
              willChange: 'transform',
            }}
          >
            {!imageLoaded && (
              <img
                src={currentPhoto.thumbnailUrl}
                alt={currentPhoto.filename}
                className="max-w-[90vw] max-h-[80vh] select-none object-contain"
                draggable={false}
                style={{ filter: 'blur(4px)' }}
              />
            )}
            <img
              ref={imageRef}
              src={displayUrl}
              alt={currentPhoto.filename}
              className="max-w-[90vw] max-h-[80vh] select-none object-contain"
              draggable={false}
              style={{
                display: imageLoaded ? 'block' : 'none',
                borderRadius: '4px',
              }}
              onLoad={handleImageLoad}
            />
          </div>
        </div>

        <PhotoSlide
          photo={nextPhoto}
          offset={`calc(100% + ${swipeOffset}px)`}
          transitioning={isSlideTransitioning}
        />
      </div>

      <div className="pointer-events-none absolute bottom-4 left-1/2 z-40 flex max-w-[90vw] -translate-x-1/2 flex-col items-center">
        {/* EXIF info bar */}
        {currentPhoto?.exif && <ExifDisplay exif={currentPhoto.exif} />}

        {/* Counter */}
        <div
          className="mt-2 text-white/50 text-sm"
          style={{ fontVariantNumeric: 'tabular-nums' }}
        >
          {currentIndex + 1} / {total}
        </div>
      </div>
    </div>
  );
};

function PhotoSlide({
  photo,
  offset,
  transitioning,
}: {
  photo: PhotoItem | null;
  offset: string;
  transitioning: boolean;
}) {
  return (
    <div
      className="pointer-events-none absolute inset-0 flex items-center justify-center"
      aria-hidden="true"
      style={{
        transform: `translate3d(${offset}, 0, 0)`,
        transition: transitioning ? 'transform 220ms ease-out' : 'none',
        willChange: 'transform',
      }}
    >
      {photo && (
        <img
          src={getDisplayUrl(photo)}
          alt=""
          className="max-w-[90vw] max-h-[80vh] select-none object-contain"
          draggable={false}
          style={{ borderRadius: '4px' }}
        />
      )}
    </div>
  );
}

// EXIF display sub-component: only renders non-null fields
function ExifDisplay({ exif }: { exif: ExifData }) {
  const parts: string[] = [];

  if (exif.cameraModel) parts.push(exif.cameraModel);
  if (exif.focalLength) parts.push(`${exif.focalLength}mm`);
  if (exif.aperture) parts.push(`f/${exif.aperture}`);
  if (exif.shutterSpeed) parts.push(`${exif.shutterSpeed}s`);
  if (exif.iso) parts.push(`ISO ${exif.iso}`);

  if (parts.length === 0) return null;

  return (
    <div
      className="mt-3 flex flex-wrap justify-center gap-3 text-sm text-white/70"
      style={{
        padding: '8px 16px',
        background: 'rgba(255,255,255,0.08)',
        borderRadius: '8px',
        backdropFilter: 'blur(8px)',
      }}
    >
      {parts.map((part, i) => (
        <span key={i} className="whitespace-nowrap">
          {part}
        </span>
      ))}
    </div>
  );
}

export default PhotoViewer;
