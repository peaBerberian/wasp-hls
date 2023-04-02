(()=>{function x(r){throw new Error("Unreachable path taken")}function b(){}var Ie=0,F=class extends W{constructor(){super();this.error=b,this.warn=b,this.info=b,this.debug=b,this._currentLevel=Ie}setLevel(t){let n=t<0||t>4?0:t;this._currentLevel=n,this.error=n>=1?console.error.bind(console):b,this.warn=n>=2?console.warn.bind(console):b,this.info=n>=3?console.info.bind(console):b,this.debug=n>=4?console.debug.bind(console):b,this.trigger("onLogLevelChange",n)}getLevel(){return this._currentLevel}hasLevel(t){return t>=this._currentLevel}},Te=new F,o=Te;var W=class{constructor(){this._listeners={}}addEventListener(e,t){let n=this._listeners[e];Array.isArray(n)?n.push(t):this._listeners[e]=[t]}removeEventListener(e,t){if(e===void 0){this._listeners={};return}let n=this._listeners[e];if(!Array.isArray(n))return;if(t===void 0){delete this._listeners[e];return}let a=n.indexOf(t);a!==-1&&n.splice(a,1),n.length===0&&delete this._listeners[e]}trigger(e,t){let n=this._listeners[e];!Array.isArray(n)||n.slice().forEach(a=>{try{a(t)}catch(s){o.error("EventEmitter: listener error",s instanceof Error?s:null)}})}};function H(){let r="",e=-1;return function(){return e++,e>=Number.MAX_SAFE_INTEGER&&(r+="0",e=0),r+String(e)}}var Pe=new TextDecoder("utf-8",{ignoreBOM:!0,fatal:!0});Pe.decode();var Qe=new Uint8Array;var D=new TextEncoder("utf-8"),Ge=typeof D.encodeInto=="function"?function(r,e){return D.encodeInto(r,e)}:function(r,e){let t=D.encode(r);return e.set(t),{read:r.length,written:t.length}};var $e=new Int32Array;var Xe=new Float64Array;var Ye=new Uint32Array;var _=Object.freeze({Init:0,0:"Init",RegularInterval:1,1:"RegularInterval",Seeking:2,2:"Seeking",Seeked:3,3:"Seeked",LoadedData:4,4:"LoadedData",LoadedMetadata:5,5:"LoadedMetadata",CanPlay:6,6:"CanPlay",CanPlayThrough:7,7:"CanPlayThrough",Ended:8,8:"Ended",Pause:9,9:"Pause",Play:10,10:"Play",RateChange:11,11:"RateChange",Stalled:12,12:"Stalled"}),Je=Object.freeze({MultivariantPlaylist:0,0:"MultivariantPlaylist",MediaPlaylist:1,1:"MediaPlaylist"}),O=Object.freeze({NoSupportedVariant:0,0:"NoSupportedVariant",UnfoundLockedVariant:1,1:"UnfoundLockedVariant",MediaSourceAttachmentError:2,2:"MediaSourceAttachmentError",Unknown:3,3:"Unknown"}),M=Object.freeze({AlreadyCreatedWithSameType:0,0:"AlreadyCreatedWithSameType",CantPlayType:1,1:"CantPlayType",EmptyMimeType:2,2:"EmptyMimeType",MediaSourceIsClosed:3,3:"MediaSourceIsClosed",NoMediaSourceAttached:4,4:"NoMediaSourceAttached",QuotaExceededError:5,5:"QuotaExceededError",Unknown:6,6:"Unknown"}),m=Object.freeze({Timeout:0,0:"Timeout",Status:1,1:"Status",Error:2,2:"Error",Other:3,3:"Other"}),Ke=Object.freeze({NoMediaSourceAttached:0,0:"NoMediaSourceAttached",UnknownError:1,1:"UnknownError"}),Ze=Object.freeze({NoMediaSourceAttached:0,0:"NoMediaSourceAttached",UnknownError:1,1:"UnknownError"}),er=Object.freeze({UnknownError:0,0:"UnknownError",NoContentLoaded:1,1:"NoContentLoaded"}),rr=Object.freeze({SourceBufferNotFound:0,0:"SourceBufferNotFound",UnknownError:1,1:"UnknownError"}),tr=Object.freeze({NoMediaSourceAttached:0,0:"NoMediaSourceAttached",UnknownError:1,1:"UnknownError"}),nr=Object.freeze({MissingExtM3uHeader:0,0:"MissingExtM3uHeader",MultivariantPlaylistWithoutVariant:1,1:"MultivariantPlaylistWithoutVariant",MissingUriLineAfterVariant:2,2:"MissingUriLineAfterVariant",UnableToReadVariantUri:3,3:"UnableToReadVariantUri",VariantMissingBandwidth:4,4:"VariantMissingBandwidth",InvalidValue:5,5:"InvalidValue",MediaTagMissingType:6,6:"MediaTagMissingType",MediaTagMissingName:7,7:"MediaTagMissingName",MediaTagMissingGroupId:8,8:"MediaTagMissingGroupId",UnableToReadLine:9,9:"UnableToReadLine",Unknown:10,10:"Unknown"}),U=Object.freeze({UnparsableExtInf:0,0:"UnparsableExtInf",UriMissingInMap:1,1:"UriMissingInMap",MissingTargetDuration:2,2:"MissingTargetDuration",UriWithoutExtInf:3,3:"UriWithoutExtInf",UnparsableByteRange:4,4:"UnparsableByteRange",Unknown:5,5:"Unknown"}),ar=Object.freeze({NoMediaSourceAttached:0,0:"NoMediaSourceAttached",MediaSourceIsClosed:1,1:"MediaSourceIsClosed",QuotaExceededError:2,2:"QuotaExceededError",TypeNotSupportedError:3,3:"TypeNotSupportedError",EmptyMimeType:4,4:"EmptyMimeType",UnknownError:5,5:"UnknownError"}),C=Object.freeze({NoResource:0,0:"NoResource",NoSourceBuffer:1,1:"NoSourceBuffer",TransmuxerError:2,2:"TransmuxerError",UnknownError:3,3:"UnknownError"}),Q=Object.freeze({BufferFull:0,0:"BufferFull",UnknownError:1,1:"UnknownError"}),or=Object.freeze({Init:0,0:"Init",Seeked:1,1:"Seeked",Seeking:2,2:"Seeking",Ended:3,3:"Ended",ReadyStateChanged:4,4:"ReadyStateChanged",RegularInterval:5,5:"RegularInterval",Error:6,6:"Error"}),ir=Object.freeze({MediaPlaylistRefresh:0,0:"MediaPlaylistRefresh",RetryRequest:1,1:"RetryRequest"}),sr=Object.freeze({Error:0,0:"Error",Warn:1,1:"Warn",Info:2,2:"Info",Debug:3,3:"Debug"}),A=Object.freeze({Audio:0,0:"Audio",Video:1,1:"Video"}),R=Object.freeze({Closed:0,0:"Closed",Ended:1,1:"Ended",Open:2,2:"Open"});var xe={bufferGoal:15,segmentRequestTimeout:2e4,segmentBackoffBase:300,segmentBackoffMax:2e3,multiVariantPlaylistRequestTimeout:15e3,multiVariantPlaylistBackoffBase:300,multiVariantPlaylistBackoffMax:2e3,mediaPlaylistRequestTimeout:15e3,mediaPlaylistBackoffBase:300,mediaPlaylistBackoffMax:2e3},X=xe;var c={AlreadyInitializedError:"AlreadyInitializedError",WasmRequestError:"WasmRequestError",UnknownInitializationError:"UnknownInitializationError",SegmentBadHttpStatus:"SegmentBadHttpStatus",SegmentRequestTimeout:"SegmentRequestTimeout",SegmentRequestError:"SegmentRequestError",SegmentRequestOtherError:"SegmentRequestOtherError",MultivariantPlaylistBadHttpStatus:"MultivariantPlaylistBadHttpStatus",MultivariantPlaylistRequestTimeout:"MultivariantPlaylistRequestTimeout",MultivariantPlaylistRequestError:"MultivariantPlaylistRequestError",MultivariantPlaylistRequestOtherError:"MultivariantPlaylistRequestOtherError",MediaPlaylistBadHttpStatus:"MediaPlaylistBadHttpStatus",MediaPlaylistRequestTimeout:"MediaPlaylistRequestTimeout",MediaPlaylistRequestError:"MediaPlaylistRequestError",MediaPlaylistRequestOtherError:"MediaPlaylistRequestOtherError",MediaSourceAttachmentError:"MediaSourceAttachmentError",NoSupportedVariant:"NoSupportedVariant",UnfoundLockedVariant:"UnfoundLockedVariant",Unknown:"Unknown",SourceBufferCantPlayType:"SourceBufferCantPlayType",SourceBufferCreationOtherError:"SourceBufferCreationOtherError",MultivariantPlaylistMissingExtM3uHeader:"MultivariantPlaylistMissingExtM3uHeader",MultivariantPlaylistWithoutVariant:"MultivariantPlaylistWithoutVariant",MultivariantPlaylistMissingUriLineAfterVariant:"MultivariantPlaylistMissingUriLineAfterVariant",MultivariantPlaylistVariantMissingBandwidth:"MultivariantPlaylistVariantMissingBandwidth",MultivariantPlaylistInvalidValue:"MultivariantPlaylistInvalidValue",MultivariantPlaylistMediaTagMissingType:"MultivariantPlaylistMediaTagMissingType",MultivariantPlaylistMediaTagMissingName:"MultivariantPlaylistMediaTagMissingName",MultivariantPlaylistMediaTagMissingGroupId:"MultivariantPlaylistMediaTagMissingGroupId",MultivariantPlaylistOtherParsingError:"MultivariantPlaylistOtherParsingError",MediaPlaylistUnparsableExtInf:"MediaPlaylistUnparsableExtInf",MediaPlaylistUriMissingInMap:"MediaPlaylistUriMissingInMap",MediaPlaylistMissingTargetDuration:"MediaPlaylistMissingTargetDuration",MediaPlaylistUriWithoutExtInf:"MediaPlaylistUriWithoutExtInf",MediaPlaylistUnparsableByteRange:"MediaPlaylistUnparsableByteRange",MediaPlaylistOtherParsingError:"MediaPlaylistOtherParsingError",SegmentTransmuxingError:"SegmentTransmuxingError",SegmentParsingOtherError:"SegmentParsingOtherError",SourceBufferFullError:"SourceBufferFullError",SourceBufferAppendError:"SourceBufferAppendError",SourceBufferRemoveError:"SourceBufferRemoveError",SourceBufferOtherError:"SourceBufferOtherError"};var y=class extends Error{constructor(t,n,a){super();switch(Object.setPrototypeOf(this,y.prototype),this.name="WaspInitializationError",t){case 0:this.code=c.AlreadyInitializedError;break;case 1:this.code=c.WasmRequestError;break;case 2:this.code=c.UnknownInitializationError;break}this.globalCode=this.code,this.wasmHttpStatus=n,this.message=a}};var S=class extends Error{constructor(t,n,a){super();switch(Object.setPrototypeOf(this,S.prototype),this.name="WaspMediaPlaylistParsingError",n){case U.MissingTargetDuration:this.code="MediaPlaylistMissingTargetDuration";break;case U.UnparsableByteRange:this.code="MediaPlaylistUnparsableByteRange";break;case U.UnparsableExtInf:this.code="MediaPlaylistUnparsableExtInf";break;case U.UriMissingInMap:this.code="MediaPlaylistUriMissingInMap";break;case U.UriWithoutExtInf:this.code="MediaPlaylistUriWithoutExtInf";break;case U.Unknown:this.code="MediaPlaylistOtherParsingError";break;default:this.code="MediaPlaylistOtherParsingError";break}this.globalCode=this.code,this.mediaType=t,this.message=a??"Unknown error when parsing a Media Playlist"}};var k=class extends Error{constructor(t,n,a,s,i){super();Object.setPrototypeOf(this,k.prototype),this.name="WaspMediaPlaylistRequestError",this.mediaType=s,this.url=t;let u=i;switch(n){case m.Status:u=u??(a===void 0?"A Media Playlist HTTP(S) request(s) responded with an invalid status":`A Media Playlist HTTP(S) request(s) responded with a ${a} status`),this.code=c.MediaPlaylistBadHttpStatus;break;case m.Timeout:u=u??"A Media Playlist HTTP(S) request(s) did not respond",this.code=c.MediaPlaylistRequestTimeout;break;case m.Error:u=u??"A Media Playlist HTTP(S) request(s) failed due to an error.",this.code=c.MediaPlaylistRequestError;break;case m.Other:u=u??"A Media Playlist HTTP(S) request(s) failed for an unknown reason.",this.code=c.MediaPlaylistRequestOtherError;break;default:this.code=c.MediaPlaylistRequestOtherError;break}this.globalCode=this.code,this.message=u??"An error arised while trying to perform a segment request"}};var E=class extends Error{constructor(t,n){super();switch(Object.setPrototypeOf(this,E.prototype),t){case 0:this.code="MultivariantPlaylistMissingExtM3uHeader";break;case 1:this.code="MultivariantPlaylistWithoutVariant";break;case 2:this.code="MultivariantPlaylistMissingUriLineAfterVariant";break;case 4:this.code="MultivariantPlaylistVariantMissingBandwidth";break;case 5:this.code="MultivariantPlaylistInvalidValue";break;case 6:this.code="MultivariantPlaylistMediaTagMissingType";break;case 7:this.code="MultivariantPlaylistMediaTagMissingName";break;case 8:this.code="MultivariantPlaylistMediaTagMissingGroupId";break;case 10:this.code="MultivariantPlaylistOtherParsingError";break;default:this.code="MultivariantPlaylistOtherParsingError";break}this.globalCode=this.code,this.name="WaspMultivariantPlaylistParsingError",this.message=n??"Unknown error when parsing the Multivariant Playlist"}};var w=class extends Error{constructor(t,n,a,s){super();Object.setPrototypeOf(this,w.prototype),this.name="WaspMultivariantPlaylistRequestError",this.url=t;let i=s;switch(n){case m.Status:i=i??(a===void 0?"A Multivariant Playlist HTTP(S) request(s) responded with an invalid status":`A Multivariant Playlist HTTP(S) request(s) responded with a ${a} status`),this.code=c.MultivariantPlaylistBadHttpStatus;break;case m.Timeout:i=i??"A Multivariant Playlist HTTP(S) request(s) did not respond",this.code=c.MultivariantPlaylistRequestTimeout;break;case m.Error:i=i??"A Multivariant Playlist HTTP(S) request(s) failed due to an error.",this.code=c.MultivariantPlaylistRequestError;break;case m.Other:i=i??"A Multivariant Playlist HTTP(S) request(s) failed for an unknown reason.",this.code=c.MultivariantPlaylistRequestOtherError;break;default:this.code=c.MultivariantPlaylistRequestOtherError;break}this.globalCode=this.code,this.message=i??"An error arised while trying to perform a segment request"}};var h=class extends Error{constructor(t,n){super();switch(Object.setPrototypeOf(this,h.prototype),this.name="WaspOtherError",t){case O.MediaSourceAttachmentError:this.code=c.MediaSourceAttachmentError;break;case O.UnfoundLockedVariant:this.code=c.UnfoundLockedVariant;break;case O.NoSupportedVariant:this.code=c.NoSupportedVariant;break;default:this.code=c.Unknown;break}this.globalCode=this.code,this.message=n??"Unknown error"}};var I=class extends Error{constructor(t,n,a){super();switch(Object.setPrototypeOf(this,I.prototype),this.name="WaspSegmentParsingError",t){case C.TransmuxerError:this.code="SegmentTransmuxingError";break;case C.NoResource:case C.NoSourceBuffer:case C.UnknownError:this.code="SegmentParsingOtherError";break;default:this.code="SegmentParsingOtherError";break}this.globalCode=this.code,this.mediaType=n,this.message=a??"Unknown error when parsing a segment"}};var T=class extends Error{constructor(t,n){super();Object.setPrototypeOf(this,T.prototype),this.name="WaspSegmentRequestError",this.url=t.url,this.isInit=t.isInit;let a=n,{mediaType:s}=t,i=Ue(s)+" segment";switch(t.reason){case m.Status:a=a??(t.status===void 0?`${i}'s HTTP(S) request(s) responded with an invalid status`:`${i}'s HTTP(S) request(s) responded with a ${t.status} status`),this.code=c.SegmentBadHttpStatus;break;case m.Timeout:a=a??`${i}'s HTTP(S) request(s) did not respond`,this.code=c.SegmentRequestTimeout;break;case m.Error:a=a??`${i}'s HTTP(S) request(s) failed due to an error.`,this.code=c.SegmentRequestError;break;case m.Other:a=a??`${i}'s HTTP(S) request(s) failed for an unknown reason.`,this.code=c.SegmentRequestOtherError;break;default:this.code=c.SegmentRequestOtherError;break}this.globalCode=this.code,this.message=a??"An error arised while trying to perform a segment request"}};function Ue(r){switch(r){case A.Audio:return"An audio";case A.Video:return"A video"}throw new Error("Unknown MediaType")}var P=class extends Error{constructor(t,n,a){super();switch(Object.setPrototypeOf(this,P.prototype),this.name="WaspSourceBufferCreationError",this.mediaType=n,t){case M.CantPlayType:this.code="SourceBufferCantPlayType";break;case M.AlreadyCreatedWithSameType:case M.EmptyMimeType:case M.MediaSourceIsClosed:case M.NoMediaSourceAttached:case M.QuotaExceededError:case M.Unknown:this.code="SourceBufferCantPlayType";break;default:this.code="SourceBufferCreationOtherError"}this.globalCode=this.code,this.message=a??"Unknown error when creating SourceBuffer"}};var v=class extends Error{constructor(t,n,a,s){super();switch(Object.setPrototypeOf(this,v.prototype),this.name="WaspSourceBufferError",this.mediaType=a,n){case null:this.code="SourceBufferRemoveError";break;case Q.BufferFull:this.code="SourceBufferFullError";break;case Q.UnknownError:this.code="SourceBufferAppendError";break;default:this.code="SourceBufferOtherError";break}this.globalCode=this.code,this.message=s??"Unknown error"}};function l(r,e,t){o.debug("--> sending to worker:",e.type),t===void 0?r.postMessage(e):r.postMessage(e,t)}var G=(a=>(a.Stopped="Stopped",a.Loading="Loading",a.Loaded="Loaded",a.Error="Error",a))(G||{});var Ae='video/mp2t;codecs="avc1.4D401F"',Be=navigator.userAgent.toLowerCase().indexOf("firefox")!==-1;function J(){return Be?!1:typeof MediaSource=="function"&&MediaSource.isTypeSupported(Ae)}function B(r,e){return r instanceof Error?{message:r.message,name:r.name}:{message:e,name:void 0}}function N(r,e){r.stopPlaybackObservations?.(),r.loadingAborter?.abort(),e!==null&&l(e,{type:"stop",value:{contentId:r.contentId}})}function K(r,e){return new Promise((t,n)=>{r.readyState>=HTMLMediaElement.HAVE_ENOUGH_DATA&&t(),e.addEventListener("abort",s),r.addEventListener("canplay",a);function a(){r.removeEventListener("canplay",a),e.removeEventListener("abort",s),t()}function s(){r.removeEventListener("canplay",a),e.removeEventListener("abort",s),e.reason!==null?n(e.reason):n(new Error("The loading operation was aborted"))}})}function Z(r){let{textTracks:e}=r;if(e!=null){for(let t=0;t<e.length;t++)e[t].mode="disabled";if(r.hasChildNodes()){let{childNodes:t}=r;for(let n=t.length-1;n>=0;n--)if(t[n].nodeName==="track")try{r.removeChild(t[n])}catch(a){let s=a instanceof Error?a.toString():"Unknown Error";o.warn("Unable to remove track element from media element",s)}}}r.src="",r.removeAttribute("src")}var j=class{constructor(e){this._sourceBuffer=e,this._queue=[],this._pendingTask=null;let t=setInterval(()=>{this._flush()},2e3),n=this._onPendingTaskError.bind(this),a=()=>{this._flush()};e.addEventListener("error",n),e.addEventListener("updateend",a),this._dispose=[()=>{clearInterval(t),e.removeEventListener("error",n),e.removeEventListener("updateend",a)}]}push(e){return o.debug("QSB: receiving order to push data to the SourceBuffer"),this._addToQueue({type:0,value:e})}removeBuffer(e,t){return o.debug("QSB: receiving order to remove data from the SourceBuffer",e,t),this._addToQueue({type:1,value:{start:e,end:t}})}getBufferedRanges(){return this._sourceBuffer.buffered}dispose(){for(this._dispose.forEach(e=>e()),this._pendingTask!==null&&(this._pendingTask.reject(new Error("QueuedSourceBuffer Cancelled")),this._pendingTask=null);this._queue.length>0;){let e=this._queue.shift();e!==void 0&&e.reject(new Error("QueuedSourceBuffer Cancelled"))}}_onPendingTaskError(e){let t=e instanceof Error?e:new Error("An unknown error occured when doing operations on the SourceBuffer");this._pendingTask!=null&&this._pendingTask.reject(t)}_addToQueue(e){return new Promise((t,n)=>{let a=this._queue.length===0&&this._pendingTask===null,s={resolve:t,reject:n,...e};this._queue.push(s),a&&this._flush()})}_flush(){if(!this._sourceBuffer.updating){if(this._pendingTask!==null){let e=this._pendingTask,{resolve:t}=e;return this._pendingTask=null,t(),this._flush()}else{let e=this._queue.shift();if(e===void 0)return;this._pendingTask=e}try{switch(this._pendingTask.type){case 0:let e=this._pendingTask.value;if(e===void 0){this._flush();return}o.debug("QSB: pushing data"),this._sourceBuffer.appendBuffer(e);break;case 1:let{start:t,end:n}=this._pendingTask.value;o.debug("QSB: removing data from SourceBuffer",t,n),this._sourceBuffer.remove(t,n);break;default:x(this._pendingTask)}}catch(e){this._onPendingTaskError(e)}}}};function L(r){let e=new Float64Array(r.length*2);for(let t=0;t<r.length;t++){let n=t*2;e[n]=r.start(t),e[n+1]=r.end(t)}return e}var Le=[["seeking",_.Seeking],["seeked",_.Seeked],["loadedmetadata",_.LoadedMetadata],["loadeddata",_.LoadedData],["canplay",_.CanPlay],["canplaythrough",_.CanPlayThrough],["ended",_.Ended],["pause",_.Pause],["play",_.Play],["ratechange",_.RateChange],["stalled",_.Stalled]];function $(r,e){let t=!1,n,a=Le.map(([i,u])=>{r.addEventListener(i,f);function f(){s(u)}return()=>r.removeEventListener(i,f)});return Promise.resolve().then(()=>s(_.Init)),()=>{t||(t=!0,a.forEach(i=>i()),a.length=0,n!==void 0&&(clearTimeout(n),n=void 0))};function s(i){if(t)return;n!==void 0&&(clearTimeout(n),n=void 0);let u=L(r.buffered),{currentTime:f,readyState:g,paused:d,seeking:p,ended:V,duration:z}=r;e({reason:i,currentTime:f,readyState:g,buffered:u,paused:d,seeking:p,ended:V,duration:z}),n=window.setTimeout(()=>{if(t){n=void 0;return}s(_.RegularInterval)},1e3)}}function ee(r,e,t){if(e?.contentId!==r.value.contentId){o.info("API: Ignoring MediaSource attachment due to wrong `contentId`");return}if(e.stopPlaybackObservations!==null&&(e.stopPlaybackObservations(),e.stopPlaybackObservations=null),r.value.handle!==void 0)t.srcObject=r.value.handle;else if(r.value.src!==void 0)t.src=r.value.src;else throw new Error('Unexpected "attach-media-source" message: missing source');e.mediaSourceId=r.value.mediaSourceId,e.mediaSource=null,e.disposeMediaSource=()=>{r.value.src!==void 0&&URL.revokeObjectURL(r.value.src)},e.sourceBuffers=[],e.stopPlaybackObservations=null}function re(r,e,t){if(e?.mediaSourceId!==r.value.mediaSourceId){o.info("API: Ignoring seek due to wrong `mediaSourceId`");return}try{t.currentTime=r.value.position}catch(n){let a=n instanceof Error?n:"Unknown Error";o.error("Unexpected error while seeking:",a)}}function te(r,e,t){if(e?.mediaSourceId!==r.value.mediaSourceId){o.info("API: Ignoring flush due to wrong `mediaSourceId`");return}try{t.currentTime=t.currentTime-.001}catch(n){let a=n instanceof Error?n:"Unknown Error";o.error("Unexpected error while flushing:",a)}}function ne(r,e,t){if(e?.mediaSourceId!==r.value.mediaSourceId){o.info("API: Ignoring playback rate update due to wrong `mediaSourceId`");return}try{t.playbackRate=r.value.playbackRate}catch(n){let a=n instanceof Error?n:"Unknown Error";o.error("Unexpected error while changing the playback rate:",a)}}function ae(r,e,t,n){if(e?.contentId!==r.value.contentId)o.info("API: Ignoring MediaSource attachment due to wrong `contentId`");else{let{mediaSourceId:a}=r.value;try{e.disposeMediaSource?.(),e.stopPlaybackObservations?.();let s=new MediaSource,i=Oe(n,s,t,a);e.mediaSourceId=r.value.mediaSourceId,e.mediaSource=s,e.disposeMediaSource=i,e.sourceBuffers=[],e.stopPlaybackObservations=null}catch(s){let{name:i,message:u}=B(s,"Unknown error when creating the MediaSource");l(n,{type:"create-ms",value:{mediaSourceId:a,message:u,name:i}})}}}function oe(r,e,t){if(e?.mediaSourceId!==r.value.mediaSourceId){o.info("API: Ignoring duration update due to wrong `mediaSourceId`");return}try{e.mediaSource===null?o.info("API: Ignoring duration update due to no MediaSource"):e.mediaSource.duration=r.value.duration}catch(n){let{name:a,message:s}=B(n,"Unknown error when updating the MediaSource's duration"),{mediaSourceId:i}=r.value;l(t,{type:"update-ms-dur-err",value:{mediaSourceId:i,message:s,name:a}})}}function ie(r,e,t){if(e?.mediaSourceId!==r.value.mediaSourceId){o.info("API: Ignoring MediaSource clearing due to wrong `mediaSourceId`");return}try{e.disposeMediaSource?.(),Z(t)}catch(n){let a=n instanceof Error?n:"Unknown Error";o.warn("API: Error when clearing current MediaSource:",a)}}function se(r,e,t){if(e?.mediaSourceId!==r.value.mediaSourceId){o.info("API: Ignoring SourceBuffer creation due to wrong `mediaSourceId`");return}if(e.mediaSource===null){l(t,{type:"create-sb",value:{mediaSourceId:r.value.mediaSourceId,sourceBufferId:r.value.sourceBufferId,code:0,message:"No MediaSource created on the main thread.",name:void 0}});return}try{let n=e.mediaSource.addSourceBuffer(r.value.contentType),a=new j(n);e.sourceBuffers.push({sourceBufferId:r.value.sourceBufferId,queuedSourceBuffer:a})}catch(n){let{name:a,message:s}=B(n,"Unknown error when adding the SourceBuffer to the MediaSource");l(t,{type:"create-sb",value:{mediaSourceId:r.value.mediaSourceId,sourceBufferId:r.value.sourceBufferId,code:1,message:s,name:a}})}}function ue(r,e,t){if(e?.mediaSourceId!==r.value.mediaSourceId){o.info("API: Ignoring appendBuffer operation due to wrong `mediaSourceId`");return}let n=e.sourceBuffers.find(({sourceBufferId:a})=>a===r.value.sourceBufferId);if(n!==void 0){let i=function(u){let{message:f}=B(u,"Unknown error when appending data to the SourceBuffer");l(t,{type:"sb-err",value:{mediaSourceId:a,sourceBufferId:s,message:f,operation:0,isBufferFull:u instanceof Error&&u.name==="QuotaExceededError"}})},{mediaSourceId:a,sourceBufferId:s}=r.value;try{n.queuedSourceBuffer.push(r.value.data).then(()=>{let u=n.queuedSourceBuffer.getBufferedRanges();l(t,{type:"sb-s",value:{mediaSourceId:a,sourceBufferId:s,buffered:L(u)}})}).catch(i)}catch(u){i(u)}}}function de(r,e,t){if(e?.mediaSourceId!==r.value.mediaSourceId){o.info("API: Ignoring removeBuffer operation due to wrong `mediaSourceId`");return}let n=e.sourceBuffers.find(({sourceBufferId:a})=>a===r.value.sourceBufferId);if(n!==void 0){let i=function(u){let{message:f}=B(u,"Unknown error when removing data to the SourceBuffer");l(t,{type:"sb-err",value:{mediaSourceId:a,sourceBufferId:s,message:f,operation:1,isBufferFull:!1}})},{mediaSourceId:a,sourceBufferId:s}=r.value;try{n.queuedSourceBuffer.removeBuffer(r.value.start,r.value.end).then(()=>{let u=n.queuedSourceBuffer.getBufferedRanges();l(t,{type:"sb-s",value:{mediaSourceId:a,sourceBufferId:s,buffered:L(u)}})}).catch(i)}catch(u){i(u)}}}function le(r,e,t,n){if(e?.mediaSourceId!==r.value.mediaSourceId){o.info("API: Ignoring `start-playback-observation` due to wrong `mediaSourceId`");return}e.stopPlaybackObservations!==null&&(e.stopPlaybackObservations(),e.stopPlaybackObservations=null),e.stopPlaybackObservations=$(t,a=>{let s={};for(let i of e.sourceBuffers){let u=i.queuedSourceBuffer.getBufferedRanges(),f=L(u);s[i.sourceBufferId]=f}l(n,{type:"obs",value:Object.assign(a,{sourceBuffersBuffered:s,mediaSourceId:r.value.mediaSourceId})})})}function ce(r,e){if(e?.mediaSourceId!==r.value.mediaSourceId){o.info("API: Ignoring `stop-playback-observation` due to wrong `mediaSourceId`");return}e.stopPlaybackObservations!==null&&(e.stopPlaybackObservations(),e.stopPlaybackObservations=null)}function fe(r,e,t){if(e?.mediaSourceId!==r.value.mediaSourceId)o.info("API: Ignoring `end-of-stream` due to wrong `mediaSourceId`");else{let{mediaSourceId:n}=r.value;if(e.mediaSource===null){l(t,{type:"eos-err",value:{mediaSourceId:n,code:0,message:"No MediaSource created on the main thread.",name:void 0}});return}if(e.mediaSource.readyState==="ended"){o.info("Ignoring redundant end-of-stream order");return}try{e.mediaSource.endOfStream()}catch(a){let{name:s,message:i}=B(a,"Unknown error when calling MediaSource.endOfStream()");l(t,{type:"eos-err",value:{mediaSourceId:n,code:1,message:i,name:s}})}}}function pe(r,e){if(e?.contentId!==r.value.contentId){o.info("API: Ignoring media offset update due to wrong `contentId`");return}e.mediaOffset=r.value.offset}function _e(r,e,t){return e?.mediaSourceId!==r.value.mediaSourceId?(o.info("API: Ignoring rebuffering start due to wrong `mediaSourceId`"),!1):(r.value.updatePlaybackRate&&(t.playbackRate=0),e.isRebuffering?!1:(e.isRebuffering=!0,!0))}function me(r,e,t){return e?.mediaSourceId!==r.value.mediaSourceId?(o.info("API: Ignoring rebuffering end due to wrong `mediaSourceId`"),!1):(t.playbackRate===0&&e.wantedSpeed!==0&&(t.playbackRate=e.wantedSpeed),e.isRebuffering?(e.isRebuffering=!1,!0):!1)}function ge(r,e){if(e?.contentId!==r.value.contentId)return o.info("API: Ignoring error due to wrong `contentId`"),null;e.loadingAborter!==void 0&&e.loadingAborter.abort(new Error("Could not load content due to an error")),e.disposeMediaSource?.(),e.stopPlaybackObservations?.();let t=be(r);return e.error=t,t}function be(r){switch(r.value.errorInfo.type){case"segment-request":return new T(r.value.errorInfo.value,r.value.message);case"multi-var-playlist-request":return new w(r.value.errorInfo.value.url,r.value.errorInfo.value.reason,r.value.errorInfo.value.status,r.value.message);case"media-playlist-request":return new k(r.value.errorInfo.value.url,r.value.errorInfo.value.reason,r.value.errorInfo.value.status,r.value.errorInfo.value.mediaType,r.value.message);case"other-error":return new h(r.value.errorInfo.value.code,r.value.message);case"sb-creation":return new P(r.value.errorInfo.value.code,r.value.errorInfo.value.mediaType,r.value.message);case"multi-var-playlist-parse":return new E(r.value.errorInfo.value.code,r.value.message);case"media-playlist-parse":return new S(r.value.errorInfo.value.mediaType,r.value.errorInfo.value.code,r.value.message);case"segment-parse":return new I(r.value.errorInfo.value.code,r.value.errorInfo.value.mediaType,r.value.message);case"push-segment-error":return new v(0,r.value.errorInfo.value.code,r.value.errorInfo.value.mediaType,r.value.message);case"remove-buffer-error":return new v(1,null,r.value.errorInfo.value.mediaType,r.value.message);case"unitialized":return new h(O.Unknown,r.value.message);default:x(r.value.errorInfo)}}function ye(r,e){return e?.contentId!==r.value.contentId?(o.info("API: Ignoring warning due to wrong `contentId`"),null):be(r)}function he(r,e){if(e?.contentId!==r.value.contentId){o.info("API: Ignoring warning due to wrong `contentId`");return}e.minimumPosition=r.value.minimumPosition,e.maximumPosition=r.value.maximumPosition}function ve(r,e){return e?.contentId!==r.value.contentId?(o.info("API: Ignoring warning due to wrong `contentId`"),!1):(e.variants=r.value.variants,e.audioTracks=r.value.audioTracks,!0)}function Me(r,e){return e?.contentId!==r.value.contentId?(o.info("API: Ignoring warning due to wrong `contentId`"),!1):r.value.mediaType!==A.Audio?(o.warn("API: track update for a type not handled for now"),!1):(e.currentAudioTrack=r.value.audioTrack?{id:r.value.audioTrack.current,isSelected:r.value.audioTrack.isSelected}:void 0,!0)}function Se(r,e){if(e?.contentId!==r.value.contentId)return o.info("API: Ignoring warning due to wrong `contentId`"),!1;let t=e.variants.find(n=>n.id===r.value.variantId);return t===void 0&&o.warn("API: VariantUpdate for an unfound variant"),t!==e.currVariant?(e.currVariant=t,!0):!1}function ke(r,e){if(e?.contentId!==r.value.contentId)return o.info("API: Ignoring warning due to wrong `contentId`"),!1;if(r.value.lockedVariant===null)return e.lockedVariant!==null?(e.lockedVariant=null,!0):!1;let t=e.variants.find(n=>n.id===r.value.lockedVariant);return t===void 0?(o.warn("API: VariantLockStatusChange for an unfound variant"),e.lockedVariant!==null?(e.lockedVariant=null,!0):!1):t!==e.lockedVariant?(e.lockedVariant=t,!0):!1}function Ee(r,e){return e?.contentId!==r.value.contentId?(o.info("API: Ignoring `content-stopped` due to wrong `contentId`"),!1):(e.disposeMediaSource?.(),e.stopPlaybackObservations?.(),e.loadingAborter?.abort(new Error("Content Stopped")),!0)}function Oe(r,e,t,n){e.addEventListener("sourceclose",u),e.addEventListener("sourceended",s),e.addEventListener("sourceopen",i);let a=URL.createObjectURL(e);t.src=a;function s(){l(r,{type:"ms-state",value:{mediaSourceId:n,state:R.Ended}})}function i(){l(r,{type:"ms-state",value:{mediaSourceId:n,state:R.Open}})}function u(){l(r,{type:"ms-state",value:{mediaSourceId:n,state:R.Closed}})}return()=>{if(e.removeEventListener("sourceclose",u),e.removeEventListener("sourceended",s),e.removeEventListener("sourceopen",i),URL.revokeObjectURL(a),e.readyState!=="closed"){let{readyState:f,sourceBuffers:g}=e;for(let d=g.length-1;d>=0;d--){let p=g[d];if(!p.updating)try{f==="open"&&p.abort(),e.removeSourceBuffer(p)}catch(V){let z=V instanceof Error?V:"Unknown Error";o.warn("Could not remove SourceBuffer",z)}}}t.src="",t.removeAttribute("src")}}function we(r,e){let t={};for(let n of r.value.mimeTypes)t[n]=MediaSource.isTypeSupported(n);l(e,{type:"codecs-support-upd",value:{mimeTypes:t}})}var Ce=H();var q=class extends W{constructor(t,n){super();this.videoElement=t,this.initializationStatus="Uninitialized",this.__worker__=null,this.__contentMetadata__=null,this.__logLevelChangeListener__=null,this.__destroyAbortController__=new AbortController,this.__config__={...X,...n??{}};let a=()=>{this.getPlayerState()==="Loaded"&&this.trigger("paused",null)},s=()=>{this.getPlayerState()==="Loaded"&&this.trigger("ended",null)},i=()=>{this.getPlayerState()==="Loaded"&&this.__contentMetadata__!==null&&this.trigger("playing",null)};this.videoElement.addEventListener("pause",a),this.videoElement.addEventListener("play",i),this.videoElement.addEventListener("ended",s),this.__destroyAbortController__.signal.addEventListener("abort",()=>{this.videoElement.removeEventListener("pause",a),this.videoElement.removeEventListener("play",i),this.videoElement.removeEventListener("ended",s)})}initialize(t){try{if(this.initializationStatus!=="Uninitialized")throw new Error("WaspHlsPlayer already initialized");this.initializationStatus="Initializing";let{wasmUrl:n,workerUrl:a}=t,s=b,i=b,u=new Promise((f,g)=>{s=f,i=g});return this.__startWorker__(a,n,s,i),u}catch(n){return this.initializationStatus="errored",Promise.reject(n)}}getConfig(){return this.__config__}updateConfig(t){if(this.__worker__===null)throw new Error("The Player is not initialized or disposed.");this.__config__={...this.__config__,...t},l(this.__worker__,{type:"upd-conf",value:this.__config__})}load(t){if(this.__worker__===null)throw new Error("The Player is not initialized or disposed.");this.__contentMetadata__!==null&&N(this.__contentMetadata__,this.__worker__);let n=Ce(),a=new AbortController;this.__contentMetadata__={contentId:n,mediaSourceId:null,mediaSource:null,disposeMediaSource:null,sourceBuffers:[],variants:[],audioTracks:[],currentAudioTrack:void 0,currVariant:void 0,lockedVariant:null,stopPlaybackObservations:null,isRebuffering:!1,mediaOffset:void 0,wantedSpeed:1,minimumPosition:void 0,maximumPosition:void 0,loadingAborter:a,error:null},this.trigger("playerStateChange","Loading"),l(this.__worker__,{type:"load",value:{contentId:n,url:t}})}getPlayerState(){return this.__contentMetadata__===null?"Stopped":this.__contentMetadata__.error!==null?"Error":this.__contentMetadata__.loadingAborter!==void 0?"Loading":"Loaded"}getPosition(){return this.__contentMetadata__===null?0:this.videoElement.currentTime-(this.__contentMetadata__.mediaOffset??0)}seek(t){if(this.getPlayerState()!=="Loaded")throw new Error("Cannot seek: no content loaded.");this.videoElement.currentTime=t+(this.__contentMetadata__?.mediaOffset??0)}getMediaOffset(){return this.__contentMetadata__?.mediaOffset??void 0}isPlaying(){return this.getPlayerState()==="Loaded"&&!this.videoElement.paused}isPaused(){return this.videoElement.paused}isEnded(){return this.videoElement.ended}isRebuffering(){return this.__contentMetadata__?.isRebuffering??!1}pause(){if(this.getPlayerState()!=="Loaded")throw new Error("Cannot pause: no content loaded.");this.videoElement.pause()}resume(){if(this.getPlayerState()!=="Loaded")throw new Error("Cannot resume: no content loaded.");this.videoElement.play().catch(()=>{})}stop(){if(this.__worker__===null)throw new Error("The Player is not initialized or disposed.");this.__contentMetadata__!==null&&N(this.__contentMetadata__,this.__worker__)}setSpeed(t){if(this.__worker__===null)throw new Error("The Player is not initialized or disposed.");if(this.__contentMetadata__===null||this.__contentMetadata__.mediaSourceId===null)throw new Error("No content is loaded");this.__contentMetadata__.wantedSpeed=t,l(this.__worker__,{type:"upd-speed",value:{mediaSourceId:this.__contentMetadata__.mediaSourceId,wantedSpeed:t}})}getSpeed(){return this.__contentMetadata__?.wantedSpeed??1}getMinimumPosition(){return this.__contentMetadata__?.minimumPosition}getMaximumPosition(){return this.__contentMetadata__?.maximumPosition}getError(){return this.__contentMetadata__?.error??null}getCurrentVariant(){return this.__contentMetadata__?.currVariant??void 0}getVariantList(){return this.__contentMetadata__?.variants??[]}getAudioTrackList(){return this.__contentMetadata__?.audioTracks??[]}getAudioTrack(){let t=this.__contentMetadata__?.currentAudioTrack?.id;if(t!==void 0)return this.getAudioTrackList()?.find(n=>n.id===t)}setAudioTrack(t){if(this.__worker__===null)throw new Error("The Player is not initialized or disposed.");if(this.__contentMetadata__===null)throw new Error("No content loaded");l(this.__worker__,{type:"set-audio",value:{contentId:this.__contentMetadata__.contentId,trackId:t}})}lockVariant(t){if(this.__worker__===null)throw new Error("The Player is not initialized or disposed.");if(this.__contentMetadata__===null)throw new Error("No content loaded");l(this.__worker__,{type:"lock-var",value:{contentId:this.__contentMetadata__.contentId,variantId:t}})}unlockVariant(){if(this.__worker__===null)throw new Error("The Player is not initialized or disposed.");if(this.__contentMetadata__===null)throw new Error("No content loaded");l(this.__worker__,{type:"lock-var",value:{contentId:this.__contentMetadata__.contentId,variantId:null}})}getLockedVariant(){return this.__contentMetadata__?.lockedVariant??null}dispose(){this.__destroyAbortController__.abort(),this.__worker__!==null&&(this.__contentMetadata__!==null&&N(this.__contentMetadata__,this.__worker__),l(this.__worker__,{type:"dispose",value:null}),this.__worker__=null,this.__logLevelChangeListener__!==null&&o.removeEventListener("onLogLevelChange",this.__logLevelChangeListener__),this.videoElement.src="")}__startWorker__(t,n,a,s){let i=!0,u=new Worker(t);this.__worker__=u,l(u,{type:"init",value:{hasMseInWorker:typeof MediaSource=="function"&&MediaSource.canConstructInDedicatedWorker===!0,canDemuxMpeg2Ts:J(),wasmUrl:n,logLevel:o.getLevel(),initialConfig:this.__config__}}),this.__logLevelChangeListener__!==null&&o.removeEventListener("onLogLevelChange",this.__logLevelChangeListener__),o.addEventListener("onLogLevelChange",f),this.__logLevelChangeListener__=f,u.onmessage=g=>{let{data:d}=g;if(typeof d!="object"||d===null||typeof d.type>"u"){o.error("unexpected Worker message");return}switch(d.type){case"init":this.initializationStatus="Initialized",i=!1,a();break;case"init-err":if(i){let p=new y(d.value.code,d.value.wasmHttpStatus,d.value.message??"Error while initializing the WaspHlsPlayer");i=!1,s(p)}break;case"seek":re(d,this.__contentMetadata__,this.videoElement);break;case"flush":te(d,this.__contentMetadata__,this.videoElement);break;case"upd-pbr":ne(d,this.__contentMetadata__,this.videoElement);break;case"attach-ms":ee(d,this.__contentMetadata__,this.videoElement),this.__startListeningToLoadedEvent__();break;case"create-ms":ae(d,this.__contentMetadata__,this.videoElement,u),this.__startListeningToLoadedEvent__();break;case"upd-ms-dur":oe(d,this.__contentMetadata__,u);break;case"clear-ms":ie(d,this.__contentMetadata__,this.videoElement);break;case"creat-sb":se(d,this.__contentMetadata__,u);break;case"push-sb":ue(d,this.__contentMetadata__,u);break;case"rem-sb":de(d,this.__contentMetadata__,u);break;case"start-obs":le(d,this.__contentMetadata__,this.videoElement,u);break;case"stop-obs":ce(d,this.__contentMetadata__);break;case"eos":fe(d,this.__contentMetadata__,u);break;case"media-off-upd":pe(d,this.__contentMetadata__);break;case"m-playlist":ve(d,this.__contentMetadata__)&&(this.trigger("variantListUpdate",this.getVariantList()),this.trigger("audioTrackListUpdate",this.getAudioTrackList()));break;case"track-upd":Me(d,this.__contentMetadata__)&&d.value.mediaType===A.Audio&&this.trigger("audioTrackUpdate",this.getAudioTrack());break;case"variant-upd":Se(d,this.__contentMetadata__)&&this.trigger("variantUpdate",this.getCurrentVariant());break;case"variant-lck-upd":ke(d,this.__contentMetadata__)&&this.trigger("variantLockUpdate",this.getLockedVariant());break;case"err":{let p=ge(d,this.__contentMetadata__);p!==null&&(o.error("API: sending fatal error",p),this.trigger("error",p),this.trigger("playerStateChange","Error"));break}case"warn":{let p=ye(d,this.__contentMetadata__);p!==null&&(o.warn("API: Triggering warning",p),this.trigger("warning",p));break}case"time-upd":he(d,this.__contentMetadata__);break;case"ctnt-stop":Ee(d,this.__contentMetadata__)&&(this.__contentMetadata__=null,this.trigger("playerStateChange","Stopped"));break;case"rebuf-start":_e(d,this.__contentMetadata__,this.videoElement)&&this.trigger("rebufferingStarted",null);break;case"rebuf-end":me(d,this.__contentMetadata__,this.videoElement)&&this.trigger("rebufferingEnded",null);break;case"are-types-supp":we(d,u);break;default:x(d)}},u.onerror=g=>{let d=g.error instanceof Error?g.error:"Unknown Error";o.error("API: Worker Error encountered",d),i&&s(new y(2,void 0,g.error?.message??void 0)),this.dispose()};function f(g){l(u,{type:"upd-log",value:g})}}__startListeningToLoadedEvent__(){let t=this.__contentMetadata__;t!==null&&t.loadingAborter!==void 0&&K(this.videoElement,t.loadingAborter.signal).then(()=>{this.__contentMetadata__!==null&&(this.__contentMetadata__.loadingAborter=void 0),this.trigger("playerStateChange","Loaded")},n=>{this.__contentMetadata__!==null&&(this.__contentMetadata__.loadingAborter=void 0);let a=n instanceof Error?n:"Unknown reason";o.info("Could not load content:",a)})}};var Vn=q;})();
