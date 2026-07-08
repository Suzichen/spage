import React, { useState, useEffect, useCallback, useMemo } from 'react';
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

const PhotoViewer: React.FC<PhotoViewerProps> = ({ photos, initialIndex, onClose }) => {
  const [currentIndex, setCurrentIndex] = useState(initialIndex);
  const [imageLoaded, setImageLoaded] = useState(false);

  const currentPhoto = photos[currentIndex];
  const total = photos.length;
  
  // Get the URL to display (use thumbnail for HEIC files)
  const displayUrl = useMemo(() => getDisplayUrl(currentPhoto), [currentPhoto]);

  // Reset imageLoaded when photo changes
  useEffect(() => {
    setImageLoaded(false);
  }, [currentIndex]);

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
      }
    };

    window.addEventListener('keydown', handleKeyDown);
    return () => window.removeEventListener('keydown', handleKeyDown);
  }, [onClose, handlePrev, handleNext]);

  // Click backdrop to close
  const handleBackdropClick = (e: React.MouseEvent<HTMLDivElement>) => {
    if (e.target === e.currentTarget) {
      onClose();
    }
  };

  return (
    <div
      className="fixed inset-0 z-50 flex items-center justify-center"
      onClick={handleBackdropClick}
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
        className="relative flex flex-col items-center max-w-[90vw] max-h-[90vh]"
        onClick={(e) => e.stopPropagation()}
      >
        {/* Progressive image loading: show thumbnail as placeholder */}
        <div className="relative">
          {!imageLoaded && (
            <img
              src={currentPhoto.thumbnailUrl}
              alt={currentPhoto.filename}
              className="max-w-[90vw] max-h-[80vh] object-contain"
              style={{ filter: 'blur(4px)', transition: 'filter 0.3s' }}
            />
          )}
          <img
            src={displayUrl}
            alt={currentPhoto.filename}
            className="max-w-[90vw] max-h-[80vh] object-contain"
            style={{
              display: imageLoaded ? 'block' : 'none',
              borderRadius: '4px',
            }}
            onLoad={() => setImageLoaded(true)}
          />
        </div>

        {/* EXIF info bar */}
        <ExifDisplay exif={currentPhoto.exif} />

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
