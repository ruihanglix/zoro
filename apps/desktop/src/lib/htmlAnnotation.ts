// Copyright (c) 2026 Ruihang Li and the Zoro Team
// Licensed under the AGPL-3.0 license.
// See LICENSE file in the project root for full license information.

import type { InkStroke } from "@/stores/annotationStore";

// Position format for HTML text annotations (highlight, underline, note)
export interface HtmlTextPosition {
	format: "html";
	startContainerXPath: string;
	startOffset: number;
	endContainerXPath: string;
	endOffset: number;
	textQuote: string;
	textPrefix: string;
	textSuffix: string;
	pageNumber: number;
	boundingRect: {
		x1: number;
		y1: number;
		x2: number;
		y2: number;
		width: number;
		height: number;
		pageNumber: number;
	};
	rects: [];
}

// Position format for HTML ink annotations
export interface HtmlInkPosition {
	format: "html";
	inkStrokes: InkStroke[];
	contentHeight: number;
	pageNumber: number;
	boundingRect: {
		x1: number;
		y1: number;
		x2: number;
		y2: number;
		width: number;
		height: number;
		pageNumber: number;
	};
	rects: [];
}

export type HtmlAnnotationPosition = HtmlTextPosition | HtmlInkPosition;

export function isHtmlAnnotation(position: unknown): boolean {
	return (
		typeof position === "object" &&
		position !== null &&
		(position as Record<string, unknown>).format === "html"
	);
}

/**
 * Returns the JavaScript to inject into the HTML reader iframe for annotation support.
 * Handles text selection, highlight rendering, note icons, ink drawing,
 * and bidirectional communication with the parent window via postMessage.
 */
export function getHtmlAnnotationScript(): string {
	return `
<style>
mark.zr-annotation {
  color: inherit !important;
}
mark.zr-annotation[data-annotation-type="underline"] {
  background-color: transparent !important;
}
</style>
<script>
(function() {
  var currentTool = 'cursor';
  var currentColor = '#ffe28f';
  var inkStrokeWidth = 2;
  var inkEraserActive = false;
  var annotations = {};

  // ═══════════════════ XPath utilities ═══════════════════

  function getXPath(node) {
    if (!node || node === document) return '';
    if (node === document.body) return '/html/body';
    if (node === document.documentElement) return '/html';
    if (node.nodeType === Node.TEXT_NODE) {
      var parent = node.parentNode;
      if (!parent) return '';
      var textNodes = [];
      for (var i = 0; i < parent.childNodes.length; i++) {
        if (parent.childNodes[i].nodeType === Node.TEXT_NODE) textNodes.push(parent.childNodes[i]);
      }
      var idx = textNodes.indexOf(node) + 1;
      return getXPath(parent) + '/text()[' + idx + ']';
    }
    if (node.nodeType !== Node.ELEMENT_NODE) return '';
    var parent = node.parentNode;
    if (!parent) return '';
    var sameTag = [];
    for (var i = 0; i < parent.children.length; i++) {
      if (parent.children[i].tagName === node.tagName) sameTag.push(parent.children[i]);
    }
    var idx = sameTag.indexOf(node) + 1;
    return getXPath(parent) + '/' + node.tagName.toLowerCase() + '[' + idx + ']';
  }

  function resolveXPath(xpath) {
    try {
      var result = document.evaluate(xpath, document, null, XPathResult.FIRST_ORDERED_NODE_TYPE, null);
      return result.singleNodeValue;
    } catch(e) { return null; }
  }

  // ═══════════════════ Text context extraction ═══════════════════

  function getTextContext(range) {
    var text = range.toString();
    var prefix = '';
    var suffix = '';
    try {
      var preRange = document.createRange();
      preRange.setStart(document.body, 0);
      preRange.setEnd(range.startContainer, range.startOffset);
      var preText = preRange.toString();
      prefix = preText.slice(-40);
    } catch(e) {}
    try {
      var walker = document.createTreeWalker(document.body, NodeFilter.SHOW_TEXT);
      var lastTextNode = null;
      while (walker.nextNode()) lastTextNode = walker.currentNode;
      if (lastTextNode) {
        var postRange = document.createRange();
        postRange.setStart(range.endContainer, range.endOffset);
        postRange.setEnd(lastTextNode, lastTextNode.textContent.length);
        var postText = postRange.toString();
        suffix = postText.slice(0, 40);
      }
    } catch(e) {}
    return { text: text, prefix: prefix, suffix: suffix };
  }

  // ═══════════════════ Highlight rendering ═══════════════════

  function getTextNodesInRange(range) {
    var nodes = [];
    if (range.commonAncestorContainer.nodeType === Node.TEXT_NODE) {
      nodes.push(range.commonAncestorContainer);
      return nodes;
    }
    var walker = document.createTreeWalker(range.commonAncestorContainer, NodeFilter.SHOW_TEXT);
    while (walker.nextNode()) {
      if (range.intersectsNode(walker.currentNode)) nodes.push(walker.currentNode);
    }
    return nodes;
  }

  function applyHighlight(annotationId, color, type, range) {
    var textNodes = getTextNodesInRange(range);
    if (textNodes.length === 0) return;
    var startContainer = range.startContainer;
    var startOffset = range.startOffset;
    var endContainer = range.endContainer;
    var endOffset = range.endOffset;

    for (var i = 0; i < textNodes.length; i++) {
      var textNode = textNodes[i];
      var nodeStart = (textNode === startContainer) ? startOffset : 0;
      var nodeEnd = (textNode === endContainer) ? endOffset : textNode.textContent.length;
      if (nodeStart >= nodeEnd || nodeStart >= textNode.textContent.length) continue;

      var targetNode = textNode;
      if (nodeStart > 0) {
        targetNode = textNode.splitText(nodeStart);
        nodeEnd -= nodeStart;
        if (textNode === endContainer) { endContainer = targetNode; endOffset = nodeEnd; }
      }
      if (nodeEnd < targetNode.textContent.length) {
        targetNode.splitText(nodeEnd);
      }

      var mark = document.createElement('mark');
      mark.className = 'zr-annotation';
      mark.dataset.annotationId = annotationId;
      mark.dataset.annotationType = type;
      if (type === 'underline') {
        mark.style.cssText = 'background:transparent;border-bottom:2px solid ' + color + ';padding:0;margin:0;cursor:pointer;';
      } else {
        mark.style.cssText = 'background-color:' + color + ';padding:0;margin:0;cursor:pointer;';
      }
      targetNode.parentNode.insertBefore(mark, targetNode);
      mark.appendChild(targetNode);
    }
  }

  function removeHighlight(annotationId) {
    var marks = document.querySelectorAll('mark.zr-annotation[data-annotation-id="' + annotationId + '"]');
    for (var i = marks.length - 1; i >= 0; i--) {
      var mark = marks[i];
      var parent = mark.parentNode;
      while (mark.firstChild) parent.insertBefore(mark.firstChild, mark);
      parent.removeChild(mark);
      parent.normalize();
    }
  }

  // ═══════════════════ Text search fallback ═══════════════════

  function findTextInDocument(quote, prefix, suffix) {
    var walker = document.createTreeWalker(document.body, NodeFilter.SHOW_TEXT);
    var textNodes = [];
    while (walker.nextNode()) textNodes.push(walker.currentNode);

    var fullText = '';
    var nodeMap = [];
    for (var i = 0; i < textNodes.length; i++) {
      var start = fullText.length;
      fullText += textNodes[i].textContent;
      nodeMap.push({ node: textNodes[i], globalStart: start, globalEnd: fullText.length });
    }

    var searchStart = 0;
    while (true) {
      var idx = fullText.indexOf(quote, searchStart);
      if (idx === -1) break;

      var prefixOk = true, suffixOk = true;
      if (prefix) {
        var actual = fullText.slice(Math.max(0, idx - prefix.length), idx);
        prefixOk = actual.indexOf(prefix) !== -1 || prefix.indexOf(actual) !== -1;
      }
      if (suffix) {
        var actual = fullText.slice(idx + quote.length, idx + quote.length + suffix.length);
        suffixOk = actual.indexOf(suffix) !== -1 || suffix.indexOf(actual) !== -1;
      }

      if (prefixOk && suffixOk) {
        var rangeStart = idx, rangeEnd = idx + quote.length;
        var startInfo = null, endInfo = null;
        for (var j = 0; j < nodeMap.length; j++) {
          var nm = nodeMap[j];
          if (!startInfo && rangeStart >= nm.globalStart && rangeStart < nm.globalEnd) {
            startInfo = { node: nm.node, offset: rangeStart - nm.globalStart };
          }
          if (rangeEnd > nm.globalStart && rangeEnd <= nm.globalEnd) {
            endInfo = { node: nm.node, offset: rangeEnd - nm.globalStart };
          }
        }
        if (startInfo && endInfo) {
          try {
            var range = document.createRange();
            range.setStart(startInfo.node, startInfo.offset);
            range.setEnd(endInfo.node, endInfo.offset);
            return range;
          } catch(e) {}
        }
      }
      searchStart = idx + 1;
    }
    return null;
  }

  // ═══════════════════ Restore highlight from saved position ═══════════════════

  function restoreHighlight(ann) {
    var pos = ann.position;
    if (!pos || !pos.startContainerXPath) return false;

    var startNode = resolveXPath(pos.startContainerXPath);
    var endNode = resolveXPath(pos.endContainerXPath);
    if (startNode && endNode) {
      try {
        var range = document.createRange();
        range.setStart(startNode, Math.min(pos.startOffset, startNode.textContent ? startNode.textContent.length : 0));
        range.setEnd(endNode, Math.min(pos.endOffset, endNode.textContent ? endNode.textContent.length : 0));
        var rangeText = range.toString();
        if (rangeText.length > 0) {
          // Verify range text matches the saved quote to guard against stale
          // XPaths (e.g. text nodes shifted by previously restored highlights)
          if (!pos.textQuote || rangeText === pos.textQuote) {
            applyHighlight(ann.id, ann.color, ann.type, range);
            return true;
          }
        }
      } catch(e) {}
    }

    if (pos.textQuote) {
      var found = findTextInDocument(pos.textQuote, pos.textPrefix, pos.textSuffix);
      if (found) {
        applyHighlight(ann.id, ann.color, ann.type, found);
        return true;
      }
    }
    return false;
  }

  // ═══════════════════ Note icons ═══════════════════

  function renderNoteIcon(ann) {
    removeNoteIcon(ann.id);
    var pos = ann.position;
    if (!pos) return;

    var anchorNode = null;
    if (pos.startContainerXPath) anchorNode = resolveXPath(pos.startContainerXPath);
    if (!anchorNode && pos.textQuote) {
      var found = findTextInDocument(pos.textQuote, pos.textPrefix, pos.textSuffix);
      if (found) anchorNode = found.startContainer;
    }
    if (!anchorNode) return;

    var element = anchorNode.nodeType === Node.TEXT_NODE ? anchorNode.parentNode : anchorNode;
    var icon = document.createElement('span');
    icon.className = 'zr-note-icon';
    icon.dataset.annotationId = ann.id;
    icon.style.cssText = 'display:inline-flex;align-items:center;justify-content:center;width:18px;height:18px;background:' + ann.color + ';border-radius:4px;cursor:pointer;vertical-align:text-bottom;margin:0 2px;font-size:12px;line-height:1;box-shadow:0 1px 2px rgba(0,0,0,0.15);';
    icon.innerHTML = '<svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path d="M12 20h9"/><path d="M16.5 3.5a2.121 2.121 0 0 1 3 3L7 19l-4 1 1-4L16.5 3.5z"/></svg>';
    icon.title = ann.comment || 'Note';

    icon.addEventListener('click', function(e) {
      e.stopPropagation();
      e.preventDefault();
      var rect = icon.getBoundingClientRect();
      window.parent.postMessage({
        type: 'zr-html-annotation-click',
        annotationId: ann.id,
        clientRect: { top: rect.top, left: rect.left, width: rect.width, height: rect.height }
      }, '*');
    });

    if (element.firstChild) element.insertBefore(icon, element.firstChild);
    else element.appendChild(icon);
  }

  function removeNoteIcon(annotationId) {
    var icons = document.querySelectorAll('.zr-note-icon[data-annotation-id="' + annotationId + '"]');
    for (var i = 0; i < icons.length; i++) icons[i].remove();
  }

  // ═══════════════════ Ink drawing ═══════════════════

  function pointsToSvgPath(points) {
    if (points.length === 0) return '';
    if (points.length === 1) return 'M ' + points[0].x + ' ' + points[0].y + ' L ' + points[0].x + ' ' + points[0].y;
    if (points.length === 2) return 'M ' + points[0].x + ' ' + points[0].y + ' L ' + points[1].x + ' ' + points[1].y;
    var d = 'M ' + points[0].x + ' ' + points[0].y;
    for (var i = 1; i < points.length - 1; i++) {
      var midX = (points[i].x + points[i + 1].x) / 2;
      var midY = (points[i].y + points[i + 1].y) / 2;
      d += ' Q ' + points[i].x + ' ' + points[i].y + ' ' + midX + ' ' + midY;
    }
    d += ' L ' + points[points.length - 1].x + ' ' + points[points.length - 1].y;
    return d;
  }

  var inkSvg = null;
  function getOrCreateInkSvg() {
    if (!inkSvg || !inkSvg.parentNode) {
      inkSvg = document.createElementNS('http://www.w3.org/2000/svg', 'svg');
      inkSvg.id = 'zr-ink-layer';
      inkSvg.setAttribute('class', 'zr-ink-svg');
      inkSvg.style.cssText = 'position:absolute;top:0;left:0;width:100%;pointer-events:none;z-index:9990;overflow:visible;';
      inkSvg.style.height = Math.max(document.documentElement.scrollHeight, document.body.scrollHeight) + 'px';
      if (document.body.style.position === '' || document.body.style.position === 'static') {
        document.body.style.position = 'relative';
      }
      document.body.appendChild(inkSvg);
    }
    inkSvg.style.height = Math.max(document.documentElement.scrollHeight, document.body.scrollHeight) + 'px';
    return inkSvg;
  }

  function renderSavedInk(inkAnnotations) {
    var svg = getOrCreateInkSvg();
    var old = svg.querySelectorAll('[data-saved-ink]');
    for (var i = 0; i < old.length; i++) old[i].remove();

    for (var i = 0; i < inkAnnotations.length; i++) {
      var ann = inkAnnotations[i];
      var pos = ann.position;
      if (!pos || !pos.inkStrokes) continue;
      for (var j = 0; j < pos.inkStrokes.length; j++) {
        var stroke = pos.inkStrokes[j];
        var path = document.createElementNS('http://www.w3.org/2000/svg', 'path');
        path.setAttribute('d', pointsToSvgPath(stroke.points));
        path.setAttribute('fill', 'none');
        path.setAttribute('stroke', ann.color);
        path.setAttribute('stroke-width', String(stroke.strokeWidth));
        path.setAttribute('stroke-linecap', 'round');
        path.setAttribute('stroke-linejoin', 'round');
        path.dataset.savedInk = 'true';
        path.dataset.annotationId = ann.id;
        svg.appendChild(path);
      }
    }
  }

  var inkOverlay = null;
  var inkDrawing = false;
  var inkCurrentStroke = [];
  var inkCurrentPath = null;

  function showInkOverlay() {
    if (inkOverlay) { inkOverlay.style.display = 'block'; return; }
    inkOverlay = document.createElement('div');
    inkOverlay.id = 'zr-ink-overlay';
    inkOverlay.style.cssText = 'position:fixed;top:0;left:0;width:100%;height:100%;z-index:9999;cursor:crosshair;';

    inkOverlay.addEventListener('pointerdown', function(e) {
      if (inkEraserActive) {
        handleInkErase(e);
        return;
      }
      e.preventDefault();
      inkOverlay.setPointerCapture(e.pointerId);
      inkDrawing = true;
      var scrollX = window.scrollX || document.documentElement.scrollLeft || 0;
      var scrollY = window.scrollY || document.documentElement.scrollTop || 0;
      inkCurrentStroke = [{ x: e.clientX + scrollX, y: e.clientY + scrollY }];
      var svg = getOrCreateInkSvg();
      inkCurrentPath = document.createElementNS('http://www.w3.org/2000/svg', 'path');
      inkCurrentPath.setAttribute('fill', 'none');
      inkCurrentPath.setAttribute('stroke', currentColor);
      inkCurrentPath.setAttribute('stroke-width', String(inkStrokeWidth));
      inkCurrentPath.setAttribute('stroke-linecap', 'round');
      inkCurrentPath.setAttribute('stroke-linejoin', 'round');
      svg.appendChild(inkCurrentPath);
    });

    inkOverlay.addEventListener('pointermove', function(e) {
      if (!inkDrawing) return;
      e.preventDefault();
      var scrollX = window.scrollX || document.documentElement.scrollLeft || 0;
      var scrollY = window.scrollY || document.documentElement.scrollTop || 0;
      inkCurrentStroke.push({ x: e.clientX + scrollX, y: e.clientY + scrollY });
      if (inkCurrentPath) inkCurrentPath.setAttribute('d', pointsToSvgPath(inkCurrentStroke));
    });

    inkOverlay.addEventListener('pointerup', function(e) {
      if (!inkDrawing) return;
      inkDrawing = false;
      try { inkOverlay.releasePointerCapture(e.pointerId); } catch(ex) {}

      if (inkCurrentStroke.length >= 2) {
        var x1 = Infinity, y1 = Infinity, x2 = -Infinity, y2 = -Infinity;
        for (var i = 0; i < inkCurrentStroke.length; i++) {
          var p = inkCurrentStroke[i];
          if (p.x < x1) x1 = p.x; if (p.y < y1) y1 = p.y;
          if (p.x > x2) x2 = p.x; if (p.y > y2) y2 = p.y;
        }
        window.parent.postMessage({
          type: 'zr-html-ink-stroke',
          stroke: { points: inkCurrentStroke, strokeWidth: inkStrokeWidth },
          boundingRect: { x1: x1, y1: y1, x2: x2, y2: y2 },
          contentHeight: Math.max(document.documentElement.scrollHeight, document.body.scrollHeight),
          color: currentColor
        }, '*');
      }
      if (inkCurrentPath) { inkCurrentPath.remove(); inkCurrentPath = null; }
      inkCurrentStroke = [];
    });

    document.body.appendChild(inkOverlay);
  }

  function hideInkOverlay() {
    if (inkOverlay) inkOverlay.style.display = 'none';
  }

  function handleInkErase(e) {
    var scrollX = window.scrollX || document.documentElement.scrollLeft || 0;
    var scrollY = window.scrollY || document.documentElement.scrollTop || 0;
    var px = e.clientX + scrollX;
    var py = e.clientY + scrollY;
    var threshold = 8;

    for (var id in annotations) {
      var ann = annotations[id];
      if (ann.type !== 'ink' || !ann.position || !ann.position.inkStrokes) continue;
      for (var si = 0; si < ann.position.inkStrokes.length; si++) {
        var stroke = ann.position.inkStrokes[si];
        for (var pi = 0; pi < stroke.points.length; pi++) {
          var dx = px - stroke.points[pi].x;
          var dy = py - stroke.points[pi].y;
          if (dx * dx + dy * dy < threshold * threshold) {
            window.parent.postMessage({ type: 'zr-html-ink-erase', annotationId: ann.id }, '*');
            return;
          }
        }
      }
    }
  }

  // ═══════════════════ Scroll to position (citation jump) ═══════════════════

  function scrollToPosition(position) {
    if (!position || !position.startContainerXPath) return;
    var node = resolveXPath(position.startContainerXPath);
    if (!node) return;
    var el = node.nodeType === Node.ELEMENT_NODE ? node : node.parentElement;
    if (!el) return;
    el.scrollIntoView({ behavior: 'smooth', block: 'center' });
    el.style.transition = 'outline 0.15s, box-shadow 0.15s';
    el.style.outline = '2px solid #4a6cf7';
    el.style.boxShadow = '0 0 0 3px rgba(74,108,247,0.25)';
    setTimeout(function() {
      el.style.outline = '';
      el.style.boxShadow = '';
    }, 2000);
  }

  // ═══════════════════ Scroll to annotation ═══════════════════

  function scrollToAnnotation(annotationId) {
    var marks = document.querySelectorAll('mark.zr-annotation[data-annotation-id="' + annotationId + '"]');
    var noteIcon = document.querySelector('.zr-note-icon[data-annotation-id="' + annotationId + '"]');
    var target = marks.length > 0 ? marks[0] : noteIcon;
    if (!target) return;

    target.scrollIntoView({ behavior: 'smooth', block: 'center' });
    if (marks.length > 0) {
      for (var i = 0; i < marks.length; i++) {
        marks[i].style.transition = 'outline 0.15s, box-shadow 0.15s';
        marks[i].style.outline = '2px solid #4a6cf7';
        marks[i].style.boxShadow = '0 0 6px rgba(74, 108, 247, 0.4)';
      }
      setTimeout(function() {
        var ms = document.querySelectorAll('mark.zr-annotation[data-annotation-id="' + annotationId + '"]');
        for (var i = 0; i < ms.length; i++) { ms[i].style.outline = 'none'; ms[i].style.boxShadow = 'none'; }
      }, 1500);
    } else if (noteIcon) {
      noteIcon.style.transition = 'transform 0.15s';
      noteIcon.style.transform = 'scale(1.5)';
      setTimeout(function() { if (noteIcon) noteIcon.style.transform = 'scale(1)'; }, 1500);
    }
  }

  // ═══════════════════ Text selection handler ═══════════════════

  document.addEventListener('mouseup', function(e) {
    if (currentTool === 'ink') return;
    if (e.target && e.target.closest && e.target.closest('#zr-ink-overlay')) return;

    var sel = window.getSelection();
    if (!sel || sel.isCollapsed || !sel.rangeCount) return;

    var range = sel.getRangeAt(0);
    var text = sel.toString().trim();
    if (!text) return;

    var startXPath = getXPath(range.startContainer);
    var endXPath = getXPath(range.endContainer);
    var context = getTextContext(range);
    var rect = range.getBoundingClientRect();

    var position = {
      format: 'html',
      startContainerXPath: startXPath,
      startOffset: range.startOffset,
      endContainerXPath: endXPath,
      endOffset: range.endOffset,
      textQuote: context.text,
      textPrefix: context.prefix,
      textSuffix: context.suffix,
      pageNumber: 0,
      boundingRect: { x1: 0, y1: 0, x2: 0, y2: 0, width: 1, height: 1, pageNumber: 0 },
      rects: []
    };

    var clientRect = { top: rect.top, left: rect.left, width: rect.width, height: rect.height };

    if (currentTool === 'note') {
      window.parent.postMessage({
        type: 'zr-html-note-request',
        position: position,
        selectedText: text.slice(0, 100),
        clientRect: clientRect
      }, '*');
      sel.removeAllRanges();
    } else if (currentTool === 'cursor') {
      window.parent.postMessage({
        type: 'zr-html-selection',
        position: position,
        selectedText: text,
        tool: 'cursor',
        clientRect: clientRect
      }, '*');
    } else {
      window.parent.postMessage({
        type: 'zr-html-selection',
        position: position,
        selectedText: text,
        tool: currentTool,
        clientRect: clientRect
      }, '*');
      sel.removeAllRanges();
    }
  });

  // Click on annotation marks or empty space
  document.addEventListener('click', function(e) {
    if (!e.target) return;
    var mark = e.target.closest ? e.target.closest('.zr-annotation') : null;
    if (mark && mark.dataset.annotationId) {
      var rect = mark.getBoundingClientRect();
      window.parent.postMessage({
        type: 'zr-html-annotation-click',
        annotationId: mark.dataset.annotationId,
        clientRect: { top: rect.top, left: rect.left, width: rect.width, height: rect.height }
      }, '*');
    } else {
      var noteIcon = e.target.closest ? e.target.closest('.zr-note-icon') : null;
      // Only send empty click if there's no active selection — prevents
      // the click event that fires after mouseup from immediately closing
      // the selection toolbar
      var sel = window.getSelection();
      if (!noteIcon && (!sel || sel.isCollapsed)) {
        window.parent.postMessage({ type: 'zr-html-click-empty' }, '*');
      }
    }
  });

  // ═══════════════════ Message handler from parent ═══════════════════

  window.addEventListener('message', function(e) {
    var data = e.data;
    if (!data || !data.type) return;

    switch (data.type) {
      case 'zr-html-set-tool':
        currentTool = data.tool || 'cursor';
        if (data.color) currentColor = data.color;
        if (typeof data.inkStrokeWidth === 'number') inkStrokeWidth = data.inkStrokeWidth;
        if (typeof data.inkEraserActive === 'boolean') inkEraserActive = data.inkEraserActive;
        document.body.style.cursor =
          (currentTool === 'highlight' || currentTool === 'underline') ? 'text' :
          currentTool === 'note' ? 'crosshair' : '';
        if (currentTool === 'ink') showInkOverlay();
        else hideInkOverlay();
        break;

      case 'zr-html-set-color':
        currentColor = data.color;
        break;

      case 'zr-html-restore-annotations': {
        var newAnns = data.annotations || [];
        var newIds = {};
        for (var i = 0; i < newAnns.length; i++) newIds[newAnns[i].id] = newAnns[i];

        for (var id in annotations) {
          if (!newIds[id]) {
            removeHighlight(id);
            removeNoteIcon(id);
            delete annotations[id];
          }
        }

        var inkAnns = [];
        for (var i = 0; i < newAnns.length; i++) {
          var ann = newAnns[i];
          var existing = annotations[ann.id];
          if (ann.type === 'ink') { inkAnns.push(ann); continue; }

          if (!existing) {
            annotations[ann.id] = ann;
            if (ann.type === 'highlight' || ann.type === 'underline') restoreHighlight(ann);
            else if (ann.type === 'note') renderNoteIcon(ann);
          } else if (existing.color !== ann.color || existing.type !== ann.type) {
            // For color changes, update CSS directly on existing marks instead of
            // removing and re-restoring (which can fail when XPaths are stale)
            if (existing.type === ann.type) {
              var marks = document.querySelectorAll('mark.zr-annotation[data-annotation-id="' + ann.id + '"]');
              var noteIcons = document.querySelectorAll('.zr-note-icon[data-annotation-id="' + ann.id + '"]');
              if (marks.length > 0) {
                for (var k = 0; k < marks.length; k++) {
                  if (ann.type === 'underline') {
                    marks[k].style.background = 'transparent';
                    marks[k].style.borderBottom = '2px solid ' + ann.color;
                  } else {
                    marks[k].style.backgroundColor = ann.color;
                  }
                }
              } else if (noteIcons.length > 0) {
                for (var k = 0; k < noteIcons.length; k++) {
                  noteIcons[k].style.background = ann.color;
                }
              } else {
                removeHighlight(ann.id);
                removeNoteIcon(ann.id);
                if (ann.type === 'highlight' || ann.type === 'underline') restoreHighlight(ann);
                else if (ann.type === 'note') renderNoteIcon(ann);
              }
            } else {
              removeHighlight(ann.id);
              removeNoteIcon(ann.id);
              if (ann.type === 'highlight' || ann.type === 'underline') restoreHighlight(ann);
              else if (ann.type === 'note') renderNoteIcon(ann);
            }
            annotations[ann.id] = ann;
          } else {
            annotations[ann.id] = ann;
          }
        }

        for (var i = 0; i < inkAnns.length; i++) annotations[inkAnns[i].id] = inkAnns[i];
        renderSavedInk(inkAnns);
        break;
      }

      case 'zr-html-add-highlight': {
        var ann = data.annotation;
        if (!ann) break;
        if (annotations[ann.id]) break;
        annotations[ann.id] = ann;
        if (ann.type === 'highlight' || ann.type === 'underline') restoreHighlight(ann);
        else if (ann.type === 'note') renderNoteIcon(ann);
        else if (ann.type === 'ink') {
          var allInk = [];
          for (var id in annotations) { if (annotations[id].type === 'ink') allInk.push(annotations[id]); }
          renderSavedInk(allInk);
        }
        break;
      }

      case 'zr-html-remove-annotation':
        removeHighlight(data.annotationId);
        removeNoteIcon(data.annotationId);
        delete annotations[data.annotationId];
        var allInk = [];
        for (var id in annotations) { if (annotations[id].type === 'ink') allInk.push(annotations[id]); }
        renderSavedInk(allInk);
        break;

      case 'zr-html-scroll-to':
        scrollToAnnotation(data.annotationId);
        break;

      case 'zr-html-scroll-to-position':
        scrollToPosition(data.position);
        break;

      case 'zr-html-scroll-to-heading':
        scrollToHeading(data.headingId);
        break;

      case 'zr-html-get-headings':
        sendHeadingsToParent();
        break;
    }
  });

  // ═══════════════════ Heading extraction for outline ═══════════════════

  function extractHeadings() {
    var headings = document.querySelectorAll('h1, h2, h3, h4, h5, h6');
    var result = [];
    for (var i = 0; i < headings.length; i++) {
      var el = headings[i];
      // Skip headings inside translation blocks to avoid duplicates
      if (el.closest && el.closest('.zr-translation-block')) continue;
      if (el.getAttribute('data-zotero-translation') === 'true') continue;
      // Use the original text content only (first text, not translated)
      var text = (el.textContent || '').trim();
      if (!text) continue;
      var level = parseInt(el.tagName.charAt(1), 10);
      // Ensure the heading has a stable id for scrolling
      var id = el.id || ('zr-heading-' + i);
      if (!el.id) el.id = id;
      result.push({ level: level, text: text, id: id });
    }
    return result;
  }

  function buildHeadingTree(flatHeadings) {
    var root = [];
    var stack = []; // { level, node }
    for (var i = 0; i < flatHeadings.length; i++) {
      var h = flatHeadings[i];
      var node = { level: h.level, text: h.text, id: h.id, children: [] };
      // Pop stack until we find a parent with a lower level
      while (stack.length > 0 && stack[stack.length - 1].level >= h.level) {
        stack.pop();
      }
      if (stack.length === 0) {
        root.push(node);
      } else {
        stack[stack.length - 1].node.children.push(node);
      }
      stack.push({ level: h.level, node: node });
    }
    return root;
  }

  function sendHeadingsToParent() {
    var flat = extractHeadings();
    var tree = buildHeadingTree(flat);
    window.parent.postMessage({ type: 'zr-html-headings', headings: tree }, '*');
  }

  function scrollToHeading(headingId) {
    var el = document.getElementById(headingId);
    if (!el) return;
    el.scrollIntoView({ behavior: 'smooth', block: 'start' });
    // Brief highlight effect
    el.style.transition = 'outline 0.15s, box-shadow 0.15s';
    el.style.outline = '2px solid #4a6cf7';
    el.style.boxShadow = '0 0 0 3px rgba(74,108,247,0.25)';
    setTimeout(function() {
      el.style.outline = '';
      el.style.boxShadow = '';
    }, 2000);
  }

  // ═══════════════════ Scroll reporting ═══════════════════

  var scrollThrottled = false;
  window.addEventListener('scroll', function() {
    if (scrollThrottled) return;
    scrollThrottled = true;
    requestAnimationFrame(function() {
      window.parent.postMessage({
        type: 'zr-html-scroll-info',
        scrollY: window.scrollY || document.documentElement.scrollTop || 0
      }, '*');
      scrollThrottled = false;
    });
  }, { passive: true });

  // Notify parent that annotation layer is ready
  window.parent.postMessage({ type: 'zr-html-annotation-ready' }, '*');

  // Extract and send headings after a short delay (allow DOM to settle)
  setTimeout(function() { sendHeadingsToParent(); }, 300);
})();
</script>`;
}
