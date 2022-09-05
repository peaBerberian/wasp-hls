(()=>{var h=class{constructor(e){this._sourceBuffer=e,this._queue=[],this._pendingTask=null;let t=setInterval(()=>{this._flush()},2e3),a=this._onPendingTaskError.bind(this),c=()=>{this._flush()};e.addEventListener("error",a),e.addEventListener("updateend",c),this._dispose=[()=>{clearInterval(t),e.removeEventListener("error",a),e.removeEventListener("updateend",c)}]}push(e){return console.debug("QSB: receiving order to push data to the SourceBuffer"),this._addToQueue({type:0,value:e})}removeBuffer(e,t){return console.debug("QSB: receiving order to remove data from the SourceBuffer",e,t),this._addToQueue({type:1,value:{start:e,end:t}})}getBufferedRanges(){return this._sourceBuffer.buffered}dispose(){for(this._dispose.forEach(e=>e()),this._pendingTask!==null&&(this._pendingTask.reject(new Error("QueuedSourceBuffer Cancelled")),this._pendingTask=null);this._queue.length>0;){let e=this._queue.shift();e!==void 0&&e.reject(new Error("QueuedSourceBuffer Cancelled"))}}_onPendingTaskError(e){let t=e instanceof Error?e:new Error("An unknown error occured when doing operations on the SourceBuffer");this._pendingTask!=null&&this._pendingTask.reject(t)}_addToQueue(e){return new Promise((t,a)=>{let c=this._queue.length===0&&this._pendingTask===null,n={resolve:t,reject:a,...e};this._queue.push(n),c&&this._flush()})}_flush(){if(!this._sourceBuffer.updating){if(this._pendingTask!==null){let e=this._pendingTask,{resolve:t}=e;return this._pendingTask=null,t(),this._flush()}else{let e=this._queue.shift();if(e===void 0)return;this._pendingTask=e}try{switch(this._pendingTask.type){case 0:let e=this._pendingTask.value;if(e===void 0){this._flush();return}console.debug("QSB: pushing data"),this._sourceBuffer.appendBuffer(e);break;case 1:let{start:t,end:a}=this._pendingTask.value;console.debug("QSB: removing data from SourceBuffer",t,a),this._sourceBuffer.remove(t,a);break;default:S(this._pendingTask)}}catch(e){this._onPendingTaskError(e)}}}};function S(o){throw new Error("Unreachable path taken")}var M=new TextDecoder("utf-8",{ignoreBOM:!0,fatal:!0});M.decode();var v=new TextEncoder("utf-8"),B=typeof v.encodeInto=="function"?function(o,e){return v.encodeInto(o,e)}:function(o,e){let t=v.encode(o);return e.set(t),{read:o.length,written:t.length}};var x=Object.freeze({PlayerInstanceNotFound:1,1:"PlayerInstanceNotFound",NoMediaSourceAttached:2,2:"NoMediaSourceAttached",UnknownError:3,3:"UnknownError"}),T=Object.freeze({PlayerInstanceNotFound:1,1:"PlayerInstanceNotFound",NoMediaSourceAttached:2,2:"NoMediaSourceAttached",UnknownError:3,3:"UnknownError"}),O=Object.freeze({PlayerInstanceNotFound:1,1:"PlayerInstanceNotFound",UnknownError:2,2:"UnknownError"}),D=Object.freeze({PlayerOrSourceBufferInstanceNotFound:1,1:"PlayerOrSourceBufferInstanceNotFound",GivenResourceNotFound:2,2:"GivenResourceNotFound",UnknownError:3,3:"UnknownError"}),W=Object.freeze({PlayerOrSourceBufferInstanceNotFound:1,1:"PlayerOrSourceBufferInstanceNotFound",UnknownError:2,2:"UnknownError"}),A=Object.freeze({PlayerInstanceNotFound:1,1:"PlayerInstanceNotFound",UnknownError:2,2:"UnknownError"}),L=Object.freeze({PlayerInstanceNotFound:0,0:"PlayerInstanceNotFound",NoMediaSourceAttached:1,1:"NoMediaSourceAttached",MediaSourceIsClosed:2,2:"MediaSourceIsClosed",QuotaExceededError:3,3:"QuotaExceededError",TypeNotSupportedError:4,4:"TypeNotSupportedError",EmptyMimeType:5,5:"EmptyMimeType",UnknownError:6,6:"UnknownError"}),C=Object.freeze({Init:0,0:"Init",Seeked:1,1:"Seeked",Seeking:2,2:"Seeking",ReadyStateChanged:3,3:"ReadyStateChanged",RegularInterval:4,4:"RegularInterval",Error:5,5:"Error"}),U=Object.freeze({Error:0,0:"Error",Warn:1,1:"Warn",Info:2,2:"Info",Debug:3,3:"Debug"}),z=Object.freeze({Audio:0,0:"Audio",Video:1,1:"Video"}),m=Object.freeze({Closed:0,0:"Closed",Ended:1,1:"Ended",Open:2,2:"Open"}),l=Object.freeze({Init:0,0:"Init",Seeking:1,1:"Seeking",Seeked:2,2:"Seeked",RegularInterval:3,3:"RegularInterval",LoadedData:4,4:"LoadedData",LoadedMetadata:5,5:"LoadedMetadata",CanPlay:6,6:"CanPlay",CanPlayThrough:7,7:"CanPlayThrough",Ended:8,8:"Ended",Pause:9,9:"Pause",Play:10,10:"Play",RateChange:11,11:"RateChange",Stalled:12,12:"Stalled"});function _(o,e,t){console.debug("--> sending to worker:",e.type),t===void 0?o.postMessage(e):o.postMessage(e,t)}var I=[["seeking",l.Seeking],["seeked",l.Seeked],["loadedmetadata",l.LoadedMetadata],["loadeddata",l.LoadedData],["canplay",l.CanPlay],["canplaythrough",l.CanPlayThrough],["ended",l.Ended],["pause",l.Pause],["play",l.Play],["ratechange",l.RateChange],["stalled",l.Stalled]],p=class{constructor(e){this.videoElement=e,this.initializationStatus=g.Uninitialized,this._worker=null,this._playbackData=null}initialize(e){try{this.initializationStatus=g.Initializing;let{wasmUrl:t,workerUrl:a}=e,c=k,n=k,u=new Promise((d,r)=>{c=d,n=r});return this._startWorker(a,t,c,n),u}catch(t){return this.initializationStatus=g.Errored,Promise.reject(t)}}loadContent(e){if(this._worker===null)throw new Error("The Player is not initialized or disposed.");_(this._worker,{type:"load",value:{url:e}})}seek(e){this.videoElement.currentTime=e}stop(){if(this._worker===null)throw new Error("The Player is not initialized or disposed.");this._stopObservingPlayback(),_(this._worker,{type:"stop",value:null})}dispose(){this._worker!==null&&(this._stopObservingPlayback(),_(this._worker,{type:"dispose",value:null}),this._worker=null)}_startWorker(e,t,a,c){let n=!0,u=new Worker(e);this._worker=u,_(u,{type:"init",value:{hasWorkerMse:typeof MediaSource=="function"&&MediaSource.canConstructInDedicatedWorker===!0,wasmUrl:t}}),u.onmessage=d=>{let{data:r}=d;if(typeof r!="object"||r===null||typeof r.type!="string"){console.error("unexpected Worker message");return}switch(r.type){case"initialized":this.initializationStatus=g.Initialized,n=!1,a();break;case"error":let f=new Error(r.value.message??"Unknown error");n&&(n=!1,c(f));break;case"seek":this.videoElement.currentTime=r.value;break;case"attach-media-source":r.value.handle!==void 0?this.videoElement.srcObject=r.value.handle:r.value.src!==void 0&&(this.videoElement.src=r.value.src),this._playbackData={mediaSourceId:r.value.mediaSourceId,mediaSource:null,dispose:()=>{r.value.src!==void 0&&URL.revokeObjectURL(r.value.src)},sourceBuffers:[],observationsData:null};break;case"create-media-source":{let{mediaSourceId:s}=r.value,i=new MediaSource,y=E(u,i,this.videoElement,s);this._playbackData={mediaSourceId:r.value.mediaSourceId,mediaSource:i,dispose:y,sourceBuffers:[],observationsData:null};break}case"update-media-source-duration":{let{mediaSourceId:s}=r.value;if(this._playbackData?.mediaSourceId!==s||this._playbackData.mediaSource===null)return;try{this._playbackData.mediaSource.duration=r.value.duration}catch{}break}case"clear-media-source":{if(this._playbackData?.mediaSourceId!==r.value.mediaSourceId)return;this._playbackData.dispose?.(),j(this.videoElement);break}case"create-source-buffer":{if(this._playbackData?.mediaSourceId!==r.value.mediaSourceId||this._playbackData.mediaSource===null)return;try{let s=this._playbackData.mediaSource.addSourceBuffer(r.value.contentType),i=new h(s);this._playbackData.sourceBuffers.push({sourceBufferId:r.value.sourceBufferId,queuedSourceBuffer:i})}catch{}break}case"append-buffer":{if(this._playbackData?.mediaSourceId!==r.value.mediaSourceId)return;let s=this._playbackData.sourceBuffers.find(({sourceBufferId:i})=>i===r.value.sourceBufferId);if(s===void 0||s.queuedSourceBuffer===null)return;s.queuedSourceBuffer.push(r.value.data).then(()=>{_(u,{type:"source-buffer-updated",value:{mediaSourceId:r.value.mediaSourceId,sourceBufferId:r.value.sourceBufferId}})}).catch(i=>{});break}case"remove-buffer":{if(this._playbackData?.mediaSourceId!==r.value.mediaSourceId)return;let s=this._playbackData.sourceBuffers.find(({sourceBufferId:i})=>i===r.value.sourceBufferId);if(s===void 0||s.queuedSourceBuffer===null)return;s.queuedSourceBuffer.removeBuffer(r.value.start,r.value.end).catch(i=>{});break}case"start-playback-observation":{this._startPlaybackObservation(r.value.mediaSourceId);break}case"stop-playback-observation":{if(this._playbackData?.mediaSourceId!==r.value.mediaSourceId)return;this._stopObservingPlayback();break}case"end-of-stream":if(this._playbackData?.mediaSourceId!==r.value.mediaSourceId||this._playbackData.mediaSource===null)return;try{this._playbackData.mediaSource.endOfStream()}catch{}break}},u.onerror=d=>{c(d.error)}}_startPlaybackObservation(e){if(this._playbackData?.mediaSourceId!==e||this._worker===null||this._playbackData.observationsData!==null)return;let t=this._worker,a=this.videoElement,c=I.map(([d,r])=>{a.addEventListener(d,f);function f(){u(r)}return()=>a.removeEventListener(d,f)});this._playbackData.observationsData={removeEventListeners(){c.forEach(d=>d())},timeoutId:void 0,isStopped:!1};let n=this._playbackData.observationsData;Promise.resolve().then(()=>u(l.Init));function u(d){if(n.isStopped)return;n.timeoutId!==void 0&&(clearTimeout(n.timeoutId),n.timeoutId=void 0);let r=new Float64Array(a.buffered.length*2);for(let b=0;b<a.buffered.length;b++){let w=b*2;r[w]=a.buffered.start(b),r[w+1]=a.buffered.end(b)}let{currentTime:f,readyState:s,paused:i,seeking:y}=a;_(t,{type:"observation",value:{mediaSourceId:e,reason:d,currentTime:f,readyState:s,buffered:r,paused:i,seeking:y}}),n.timeoutId=setTimeout(()=>{if(n.isStopped){n.timeoutId=void 0;return}u(l.RegularInterval)},1e3)}}_stopObservingPlayback(){this._playbackData===null||this._playbackData.observationsData===null||(this._playbackData.observationsData.isStopped=!0,this._playbackData.observationsData.removeEventListeners(),this._playbackData.observationsData.timeoutId!==void 0&&clearTimeout(this._playbackData.observationsData.timeoutId),this._playbackData.observationsData=null)}};function E(o,e,t,a){e.addEventListener("sourceclose",d),e.addEventListener("sourceended",n),e.addEventListener("sourceopen",u);let c=URL.createObjectURL(e);t.src=c;function n(){_(o,{type:"media-source-state-changed",value:{mediaSourceId:a,state:m.Ended}})}function u(){_(o,{type:"media-source-state-changed",value:{mediaSourceId:a,state:m.Open}})}function d(){_(o,{type:"media-source-state-changed",value:{mediaSourceId:a,state:m.Closed}})}return()=>{if(e.removeEventListener("sourceclose",d),e.removeEventListener("sourceended",n),e.removeEventListener("sourceopen",u),URL.revokeObjectURL(c),e.readyState!=="closed"){let{readyState:r,sourceBuffers:f}=e;for(let s=f.length-1;s>=0;s--){let i=f[s];if(!i.updating)try{r==="open"&&i.abort(),e.removeSourceBuffer(i)}catch{}}}t.src="",t.removeAttribute("src")}}var g=(n=>(n.Uninitialized="Uninitialized",n.Initializing="Initializing",n.Initialized="Initialized",n.Errored="errorred",n.Disposed="disposed",n))(g||{});function k(){}function j(o){let{textTracks:e}=o;if(e!=null){for(let t=0;t<e.length;t++)e[t].mode="disabled";if(o.hasChildNodes()){let{childNodes:t}=o;for(let a=t.length-1;a>=0;a--)if(t[a].nodeName==="track")try{o.removeChild(t[a])}catch{}}}o.src="",o.removeAttribute("src")}window.WaspHlsPlayer=p;var K=p;})();
