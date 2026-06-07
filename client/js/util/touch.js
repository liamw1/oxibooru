"use strict";

const direction = {
    NONE: null,
    LEFT: "left",
    RIGHT: "right",
    DOWN: "down",
    UP: "up",
};

function handleTouchStart(handler, evt) {
    const touchEvent = evt.touches[0];
    handler._xStart = touchEvent.clientX;
    handler._yStart = touchEvent.clientY;

    const element = handler._getScrollElement();
    if (element) {
        handler._atLeftEdge = element.scrollLeft <= 0;
        handler._atRightEdge = element.scrollLeft + element.clientWidth >= element.scrollWidth - 1; // -1 for sub-pixel rounding
    } else {
        handler._atLeftEdge = true;
        handler._atRightEdge = true;
    }
}

function handleTouchMove(handler, evt) {
    if (!handler._xStart || !handler._yStart) {
        return;
    }

    const xDirection = handler._xStart - evt.touches[0].clientX;
    const yDirection = handler._yStart - evt.touches[0].clientY;

    if (Math.abs(xDirection) > Math.abs(yDirection)) {
        if (xDirection > 0) {
            handler._direction = direction.LEFT;
        } else {
            handler._direction = direction.RIGHT;
        }
    } else if (yDirection > 0) {
        handler._direction = direction.DOWN;
    } else {
        handler._direction = direction.UP;
    }
}

function handleTouchEnd(handler) {
    switch (handler._direction) {
        case direction.NONE:
            return;
        case direction.LEFT:
            if (handler._atRightEdge) handler._swipeLeftTask();
            break;
        case direction.RIGHT:
            if (handler._atLeftEdge) handler._swipeRightTask();
            break;
        case direction.DOWN:
            handler._swipeDownTask();
            break;
        case direction.UP:
            handler._swipeUpTask();
        // no default
    }

    handler._xStart = null;
    handler._yStart = null;
}

class Touch {
    constructor(
        target,
        swipeLeft = () => { },
        swipeRight = () => { },
        swipeUp = () => { },
        swipeDown = () => { },
        getScrollElement = () => target
    ) {
        this._target = target;
        this._getScrollElement = getScrollElement;

        this._swipeLeftTask = swipeLeft;
        this._swipeRightTask = swipeRight;
        this._swipeUpTask = swipeUp;
        this._swipeDownTask = swipeDown;

        this._xStart = null;
        this._yStart = null;
        this._direction = direction.NONE;
        this._atLeftEdge = true;
        this._atRightEdge = true;

        this._target.addEventListener("touchstart", (evt) => {
            handleTouchStart(this, evt);
        });
        this._target.addEventListener("touchmove", (evt) => {
            handleTouchMove(this, evt);
        });
        this._target.addEventListener("touchend", () => {
            handleTouchEnd(this);
        });
    }
}

module.exports = Touch;
