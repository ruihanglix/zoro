import { useAnnotationStore } from "@/stores/annotationStore";
import { useCallback, useEffect, useRef, useState } from "react";

interface UseAutoscrollOptions {
	containerRef: React.RefObject<HTMLDivElement | null>;
}

interface AutoscrollState {
	active: boolean;
	anchorX: number;
	anchorY: number;
}

export function useAutoscroll({
	containerRef,
}: UseAutoscrollOptions): AutoscrollState {
	const [state, setState] = useState<AutoscrollState>({
		active: false,
		anchorX: 0,
		anchorY: 0,
	});

	const activeRef = useRef(false);
	const anchorRef = useRef({ x: 0, y: 0 });
	const mouseRef = useRef({ x: 0, y: 0 });
	const scrollContainerRef = useRef<HTMLElement | null>(null);
	const rafRef = useRef<number>(0);

	const deactivate = useCallback(() => {
		activeRef.current = false;
		setState({ active: false, anchorX: 0, anchorY: 0 });

		if (rafRef.current) {
			cancelAnimationFrame(rafRef.current);
			rafRef.current = 0;
		}

		const sc = scrollContainerRef.current;
		if (sc) {
			sc.style.userSelect = "";
			sc.style.cursor = "";
		}
		scrollContainerRef.current = null;
	}, []);

	useEffect(() => {
		const container = containerRef.current;
		if (!container) return;

		const handleMiddleDown = (e: MouseEvent) => {
			if (e.button !== 1) return;

			// Don't activate if ink or note tool is active
			const activeTool = useAnnotationStore.getState().activeTool;
			if (activeTool === "ink" || activeTool === "note") return;

			// If already active, deactivate
			if (activeRef.current) {
				deactivate();
				return;
			}

			e.preventDefault();
			e.stopPropagation();

			// Find the scroll container
			const target = e.target as HTMLElement;
			const sc = target.closest<HTMLElement>("[class*='_container_']");
			if (!sc) return;

			scrollContainerRef.current = sc;
			sc.style.userSelect = "none";
			sc.style.cursor = "default";

			// Compute anchor relative to containerRef
			const containerRect = container.getBoundingClientRect();
			const anchorX = e.clientX - containerRect.left;
			const anchorY = e.clientY - containerRect.top;

			anchorRef.current = { x: e.clientX, y: e.clientY };
			mouseRef.current = { x: e.clientX, y: e.clientY };

			activeRef.current = true;
			setState({ active: true, anchorX, anchorY });

			// Start rAF scroll loop
			let lastSyncTime = 0;
			const tick = () => {
				if (!activeRef.current) return;

				const dx = mouseRef.current.x - anchorRef.current.x;
				const dy = mouseRef.current.y - anchorRef.current.y;

				const absDx = Math.abs(dx);
				const absDy = Math.abs(dy);

				const computeSpeed = (d: number): number => {
					if (d <= 15) return 0;
					if (d <= 100) return (d - 15) * 0.15;
					return 12.75 + (d - 100) * 0.4;
				};

				const speedX = computeSpeed(absDx);
				const speedY = computeSpeed(absDy);

				const scrollEl = scrollContainerRef.current;
				if (scrollEl) {
					if (speedY > 0) {
						scrollEl.scrollTop += Math.sign(dy) * speedY;
					}
					if (speedX > 0) {
						scrollEl.scrollLeft += Math.sign(dx) * speedX;
					}

					// Dispatch mouseenter every ~150ms to keep activeSide fresh for bilingual sync
					const now = performance.now();
					if (now - lastSyncTime > 150) {
						lastSyncTime = now;
						scrollEl.dispatchEvent(
							new MouseEvent("mouseenter", { bubbles: false }),
						);
					}
				}

				rafRef.current = requestAnimationFrame(tick);
			};
			rafRef.current = requestAnimationFrame(tick);
		};

		const handleMouseMove = (e: MouseEvent) => {
			if (!activeRef.current) return;
			mouseRef.current = { x: e.clientX, y: e.clientY };
		};

		const handleExit = (e: MouseEvent | KeyboardEvent | FocusEvent) => {
			if (!activeRef.current) return;

			if (e.type === "keydown") {
				if ((e as KeyboardEvent).key !== "Escape") return;
			}

			// For mousedown/click, ignore middle button (it's handled in handleMiddleDown)
			if (e.type === "mousedown" || e.type === "click") {
				if ((e as MouseEvent).button === 1) return;
			}

			deactivate();
		};

		// Suppress left-click text selection while active
		const handleLeftDown = (e: MouseEvent) => {
			if (!activeRef.current) return;
			if (e.button === 0) {
				deactivate();
				e.preventDefault();
				e.stopPropagation();
			}
		};

		container.addEventListener("mousedown", handleMiddleDown, true);
		container.addEventListener("mousedown", handleLeftDown, true);
		document.addEventListener("mousemove", handleMouseMove);
		document.addEventListener("mousedown", handleExit);
		document.addEventListener("click", handleExit);
		document.addEventListener("keydown", handleExit);
		window.addEventListener("blur", handleExit);

		return () => {
			container.removeEventListener("mousedown", handleMiddleDown, true);
			container.removeEventListener("mousedown", handleLeftDown, true);
			document.removeEventListener("mousemove", handleMouseMove);
			document.removeEventListener("mousedown", handleExit);
			document.removeEventListener("click", handleExit);
			document.removeEventListener("keydown", handleExit);
			window.removeEventListener("blur", handleExit);

			if (rafRef.current) cancelAnimationFrame(rafRef.current);
			deactivate();
		};
	}, [containerRef, deactivate]);

	return state;
}
